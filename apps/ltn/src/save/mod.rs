mod perma;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use abstio::MapName;
use abstutil::{Counter, Timer};
use map_model::EditRoad;
use widgetry::tools::{ChooseSomething, PopupMsg, PromptInput};
use widgetry::{Choice, EventCtx, Key, Line, State, Widget};

use crate::edit::EditMode;
use crate::partition::BlockID;
use crate::{App, BrowseNeighbourhoods, Edits, Partitioning, Transition};

/// Captures all of the edits somebody makes to a map in the LTN tool. Note this is separate from
/// `map_model::MapEdits`.
#[derive(Serialize, Deserialize)]
pub struct Proposal {
    pub map: MapName,
    pub name: String,
    pub abst_version: String,

    pub partitioning: Partitioning,
    pub edits: Edits,
}

impl Proposal {
    fn from_app(app: &App) -> Self {
        Self {
            map: app.map.get_name().clone(),
            name: app
                .session
                .proposal_name
                .clone()
                .unwrap_or(String::from("existing LTNs")),
            abst_version: map_gui::tools::version().to_string(),

            partitioning: app.session.partitioning.clone(),
            edits: app.session.edits.clone(),
        }
    }

    fn make_active(self, ctx: &EventCtx, app: &mut App) {
        // First undo any one-way changes
        let mut edits = app.map.new_edits();
        for r in app.session.edits.one_ways.keys().cloned() {
            // Just revert to the original state
            edits.commands.push(app.map.edit_road_cmd(r, |new| {
                *new = EditRoad::get_orig_from_osm(app.map.get_r(r), app.map.get_config());
            }));
        }

        app.session.proposal_name = Some(self.name);
        app.session.partitioning = self.partitioning;
        app.session.edits = self.edits;
        app.session.draw_all_filters = app.session.edits.draw(ctx, &app.map);

        // Then append any new one-way changes. Edits are applied in order, so the net effect
        // should be correct.
        for (r, r_edit) in &app.session.edits.one_ways {
            edits.commands.push(app.map.edit_road_cmd(*r, move |new| {
                *new = r_edit.clone();
            }));
        }
        app.map.must_apply_edits(edits, &mut Timer::throwaway());
    }

    /// Try to load a proposal. If it fails, returns a popup message state.
    pub fn load(ctx: &mut EventCtx, app: &mut App, path: String) -> Option<Box<dyn State<App>>> {
        match Self::inner_load(ctx, app, &path) {
            Ok(()) => None,
            Err(err) => Some(PopupMsg::new_state(
                ctx,
                "Error",
                vec![
                    format!("Couldn't load proposal {}", path),
                    err.to_string(),
                    "The format of saved proposals recently changed.".to_string(),
                    "Contact dabreegster@gmail.com if you need help restoring a file.".to_string(),
                ],
            )),
        }
    }

    fn inner_load(ctx: &mut EventCtx, app: &mut App, path: &str) -> Result<()> {
        let bytes = abstio::slurp_file(path)?;
        let decoder = flate2::read::GzDecoder::new(&bytes[..]);
        let value = serde_json::from_reader(decoder)?;
        let proposal = perma::from_permanent(&app.map, value)?;

        // TODO We could try to detect if the file's partitioning (road IDs and such) still matches
        // this version of the map or not

        // When initially loading a proposal from CLI flag, the partitioning will be a placeholder.
        // Don't stash it.
        if !app.session.partitioning.is_empty() {
            stash_current_proposal(app);

            // Start a new proposal
            app.session.alt_proposals.list.push(None);
            app.session.alt_proposals.current = app.session.alt_proposals.list.len() - 1;
        }

        proposal.make_active(ctx, app);

        Ok(())
    }
}

fn stash_current_proposal(app: &mut App) {
    *app.session
        .alt_proposals
        .list
        .get_mut(app.session.alt_proposals.current)
        .unwrap() = Some(Proposal::from_app(app));
}

fn switch_to_existing_proposal(ctx: &mut EventCtx, app: &mut App, idx: usize) {
    stash_current_proposal(app);

    let proposal = app
        .session
        .alt_proposals
        .list
        .get_mut(idx)
        .unwrap()
        .take()
        .unwrap();
    app.session.alt_proposals.current = idx;

    proposal.make_active(ctx, app);
}

fn save_ui(ctx: &mut EventCtx, app: &App, preserve_state: PreserveState) -> Box<dyn State<App>> {
    let default_name = app
        .session
        .proposal_name
        .clone()
        .unwrap_or_else(String::new);
    PromptInput::new_state(
        ctx,
        "Name this proposal",
        default_name,
        Box::new(|name, ctx, app| {
            // If we overwrite an existing proposal, all hell may break loose. AltProposals state
            // and file state are not synchronized / auto-saved.
            app.session.proposal_name = Some(name.clone());

            match inner_save(app) {
                // If we changed the name, we'll want to recreate the panel
                Ok(()) => preserve_state.switch_to_state(ctx, app),
                Err(err) => Transition::Multi(vec![
                    preserve_state.switch_to_state(ctx, app),
                    Transition::Push(PopupMsg::new_state(
                        ctx,
                        "Error",
                        vec![format!("Couldn't save proposal: {}", err)],
                    )),
                ]),
            }
        }),
    )
}

fn inner_save(app: &App) -> Result<()> {
    let proposal = Proposal::from_app(app);
    let path = abstio::path_ltn_proposals(app.map.get_name(), &proposal.name);

    let json_value = perma::to_permanent(&app.map, &proposal)?;
    let mut output_buffer = Vec::new();
    let mut encoder =
        flate2::write::GzEncoder::new(&mut output_buffer, flate2::Compression::best());
    serde_json::to_writer(&mut encoder, &json_value)?;
    encoder.finish()?;
    abstio::write_raw(path, &output_buffer)
}

fn load_picker_ui(
    ctx: &mut EventCtx,
    app: &App,
    preserve_state: PreserveState,
) -> Box<dyn State<App>> {
    // Don't bother trying to filter out proposals currently loaded -- by loading twice, somebody
    // effectively makes a copy to modify a bit
    ChooseSomething::new_state(
        ctx,
        "Load which proposal?",
        // basename (and thus list_all_objects) turn "foo.json.gz" into "foo.json", so further
        // strip out the extension.
        // TODO Fix basename, but make sure nothing downstream breaks
        Choice::strings(
            abstio::list_all_objects(abstio::path_all_ltn_proposals(app.map.get_name()))
                .into_iter()
                .map(abstutil::basename)
                .collect(),
        ),
        Box::new(|name, ctx, app| {
            match Proposal::load(
                ctx,
                app,
                abstio::path_ltn_proposals(app.map.get_name(), &name),
            ) {
                Some(err_state) => Transition::Replace(err_state),
                None => preserve_state.switch_to_state(ctx, app),
            }
        }),
    )
}

pub struct AltProposals {
    // All entries are filled out, except for the current proposal being worked on
    list: Vec<Option<Proposal>>,
    current: usize,
}

impl AltProposals {
    pub fn new() -> Self {
        Self {
            list: vec![None],
            current: 0,
        }
    }

    pub fn to_widget(&self, ctx: &EventCtx, app: &App) -> Widget {
        let mut col = vec![
            Line("LTN Policy Proposals")
                .small_heading()
                .into_widget(ctx),
            Widget::row(vec![
                ctx.style().btn_outline.text("New").build_def(ctx),
                ctx.style().btn_outline.text("Load").build_def(ctx),
                ctx.style().btn_outline.text("Save").build_def(ctx),
            ]),
        ];
        for (idx, proposal) in self.list.iter().enumerate() {
            let button = if let Some(proposal) = proposal {
                ctx.style()
                    .btn_solid_primary
                    .text(format!("{} - {}", idx + 1, proposal.name))
                    .hotkey(Key::NUM_KEYS[idx])
                    .build_widget(ctx, &format!("switch to proposal {}", idx))
            } else {
                ctx.style()
                    .btn_solid_primary
                    .text(format!(
                        "{} - {}",
                        idx + 1,
                        app.session
                            .proposal_name
                            .as_ref()
                            .unwrap_or(&String::from("existing LTNs")),
                    ))
                    .disabled(true)
                    .build_def(ctx)
            };
            col.push(Widget::row(vec![
                button,
                ctx.style()
                    .btn_close()
                    .disabled(self.list.len() == 1)
                    .build_widget(ctx, &format!("hide proposal {}", idx)),
            ]));
            // If somebody tries to load too many proposals, just stop
            if idx == 9 {
                break;
            }
        }
        Widget::col(col).section(ctx)
    }

    pub fn handle_action(
        ctx: &mut EventCtx,
        app: &mut App,
        preserve_state: PreserveState,
        action: &str,
    ) -> Option<Transition> {
        match action {
            "New" => {
                stash_current_proposal(app);

                // This is expensive -- maybe we should just calculate this once and keep a copy
                // forever
                ctx.loading_screen("create new proposal", |ctx, timer| {
                    // First undo any one-way changes. This is messy to repeat here, but it's not
                    // straightforward to use make_active.
                    let mut edits = app.map.new_edits();
                    for r in app.session.edits.one_ways.keys().cloned() {
                        edits.commands.push(app.map.edit_road_cmd(r, |new| {
                            *new =
                                EditRoad::get_orig_from_osm(app.map.get_r(r), app.map.get_config());
                        }));
                    }
                    app.map.must_apply_edits(edits, timer);

                    crate::clear_current_proposal(ctx, app, timer);
                });

                // Start a new proposal
                app.session.alt_proposals.list.push(None);
                app.session.alt_proposals.current = app.session.alt_proposals.list.len() - 1;
            }
            "Load" => {
                return Some(Transition::Push(load_picker_ui(ctx, app, preserve_state)));
            }
            "Save" => {
                return Some(Transition::Push(save_ui(ctx, app, preserve_state)));
            }
            _ => {
                if let Some(x) = action.strip_prefix("switch to proposal ") {
                    let idx = x.parse::<usize>().unwrap();
                    switch_to_existing_proposal(ctx, app, idx);
                } else if let Some(x) = action.strip_prefix("hide proposal ") {
                    let idx = x.parse::<usize>().unwrap();
                    if idx == app.session.alt_proposals.current {
                        // First make sure we're not hiding the current proposal
                        switch_to_existing_proposal(ctx, app, if idx == 0 { 1 } else { idx - 1 });
                    }

                    // Remove it
                    app.session.alt_proposals.list.remove(idx);

                    // Fix up indices
                    if idx < app.session.alt_proposals.current {
                        app.session.alt_proposals.current -= 1;
                    }
                } else {
                    return None;
                }
            }
        }

        Some(preserve_state.switch_to_state(ctx, app))
    }
}

// After switching proposals, we have to recreate state
//
// To preserve per-neighborhood states, we have to transform neighbourhood IDs, which may change if
// the partitioning is different. If the boundary is a bit different, match up by all the blocks in
// the current neighbourhood.
pub enum PreserveState {
    BrowseNeighbourhoods,
    Route,
    // TODO app.session.edit_mode now has state for Shortcuts...
    Connectivity(Vec<BlockID>),
}

impl PreserveState {
    fn switch_to_state(self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self {
            PreserveState::BrowseNeighbourhoods => {
                Transition::Replace(BrowseNeighbourhoods::new_state(ctx, app))
            }
            PreserveState::Route => {
                Transition::Replace(crate::route_planner::RoutePlanner::new_state(ctx, app))
            }
            PreserveState::Connectivity(blocks) => {
                // Count which new neighbourhoods have the blocks from the original. Pick the one
                // with the most matches
                let mut count = Counter::new();
                for block in blocks {
                    count.inc(app.session.partitioning.block_to_neighbourhood(block));
                }

                if let EditMode::Shortcuts(ref mut maybe_focus) = app.session.edit_mode {
                    // TODO We should try to preserve the focused road at least, or the specific
                    // shortcut maybe.
                    *maybe_focus = None;
                }
                if let EditMode::FreehandFilters(_) = app.session.edit_mode {
                    app.session.edit_mode = EditMode::Filters;
                }

                Transition::Replace(crate::connectivity::Viewer::new_state(
                    ctx,
                    app,
                    count.max_key(),
                ))
            }
        }
    }
}
