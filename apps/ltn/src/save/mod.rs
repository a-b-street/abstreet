mod perma;
mod share;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use abstio::MapName;
use abstutil::{Counter, Timer};
use map_model::{EditRoad, Map};
use widgetry::tools::{ChooseSomething, PopupMsg, PromptInput};
use widgetry::{Choice, EventCtx, Key, State, Widget};

use crate::edit::EditMode;
use crate::partition::BlockID;
use crate::{App, Edits, Partitioning, PickArea, Transition};

pub use share::PROPOSAL_HOST_URL;

/// Captures all of the edits somebody makes to a map in the LTN tool. Note this is separate from
/// `map_model::MapEdits`.
#[derive(Clone, Serialize, Deserialize)]
pub struct Proposal {
    pub map: MapName,
    pub name: String,
    pub abst_version: String,

    pub partitioning: Partitioning,
    pub edits: Edits,
}

impl Proposal {
    fn make_active(self, ctx: &EventCtx, app: &mut App) {
        // First undo any one-way changes
        let mut edits = app.per_map.map.new_edits();
        for r in app.edits().one_ways.keys().cloned() {
            // Just revert to the original state
            edits.commands.push(app.per_map.map.edit_road_cmd(r, |new| {
                *new = EditRoad::get_orig_from_osm(
                    app.per_map.map.get_r(r),
                    app.per_map.map.get_config(),
                );
            }));
        }

        app.per_map.alt_proposals.current_proposal = self;
        app.per_map.draw_all_filters = app.edits().draw(ctx, &app.per_map.map);

        // Then append any new one-way changes. Edits are applied in order, so the net effect
        // should be correct.
        for (r, r_edit) in &app.edits().one_ways {
            edits
                .commands
                .push(app.per_map.map.edit_road_cmd(*r, move |new| {
                    *new = r_edit.clone();
                }));
        }
        app.per_map
            .map
            .must_apply_edits(edits, &mut Timer::throwaway());
    }

    /// Try to load a proposal. If it fails, returns a popup message state.
    pub fn load_from_path(
        ctx: &mut EventCtx,
        app: &mut App,
        path: String,
    ) -> Option<Box<dyn State<App>>> {
        Self::load_from_bytes(ctx, app, &path, abstio::slurp_file(path.clone()))
    }

    pub fn load_from_bytes(
        ctx: &mut EventCtx,
        app: &mut App,
        name: &str,
        bytes: Result<Vec<u8>>,
    ) -> Option<Box<dyn State<App>>> {
        match bytes.and_then(|bytes| Self::inner_load(ctx, app, bytes)) {
            Ok(()) => None,
            Err(err) => Some(PopupMsg::new_state(
                ctx,
                "Error",
                vec![
                    format!("Couldn't load proposal {}", name),
                    err.to_string(),
                    "The format of saved proposals recently changed.".to_string(),
                    "Contact dabreegster@gmail.com if you need help restoring a file.".to_string(),
                ],
            )),
        }
    }

    fn inner_load(ctx: &mut EventCtx, app: &mut App, bytes: Vec<u8>) -> Result<()> {
        let decoder = flate2::read::GzDecoder::new(&bytes[..]);
        let value = serde_json::from_reader(decoder)?;
        let proposal = perma::from_permanent(&app.per_map.map, value)?;

        // TODO We could try to detect if the file's partitioning (road IDs and such) still matches
        // this version of the map or not

        // When initially loading a proposal from CLI flag, the partitioning will be a placeholder.
        // Don't stash it.
        if !app.partitioning().is_empty() {
            stash_current_proposal(app);

            // Start a new proposal
            app.per_map.alt_proposals.list.push(None);
            app.per_map.alt_proposals.current = app.per_map.alt_proposals.list.len() - 1;
        }

        proposal.make_active(ctx, app);

        Ok(())
    }

    fn to_gzipped_bytes(&self, app: &App) -> Result<Vec<u8>> {
        let json_value = perma::to_permanent(&app.per_map.map, self)?;
        let mut output_buffer = Vec::new();
        let mut encoder =
            flate2::write::GzEncoder::new(&mut output_buffer, flate2::Compression::best());
        serde_json::to_writer(&mut encoder, &json_value)?;
        encoder.finish()?;
        Ok(output_buffer)
    }

    fn checksum(&self, app: &App) -> Result<String> {
        let bytes = self.to_gzipped_bytes(app)?;
        let mut context = md5::Context::new();
        context.consume(&bytes);
        Ok(format!("{:x}", context.compute()))
    }
}

fn stash_current_proposal(app: &mut App) {
    // TODO Could we swap and be more efficient?
    *app.per_map
        .alt_proposals
        .list
        .get_mut(app.per_map.alt_proposals.current)
        .unwrap() = Some(app.per_map.alt_proposals.current_proposal.clone());
}

fn switch_to_existing_proposal(ctx: &mut EventCtx, app: &mut App, idx: usize) {
    stash_current_proposal(app);

    let proposal = app
        .per_map
        .alt_proposals
        .list
        .get_mut(idx)
        .unwrap()
        .take()
        .unwrap();
    app.per_map.alt_proposals.current = idx;

    proposal.make_active(ctx, app);
}

fn save_ui(ctx: &mut EventCtx, app: &App, preserve_state: PreserveState) -> Box<dyn State<App>> {
    let default_name = app.per_map.alt_proposals.current_proposal.name.clone();
    PromptInput::new_state(
        ctx,
        "Name this proposal",
        default_name,
        Box::new(|name, ctx, app| {
            app.per_map.alt_proposals.current_proposal.name = name;

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
    let proposal = &app.per_map.alt_proposals.current_proposal;
    let path = abstio::path_ltn_proposals(app.per_map.map.get_name(), &proposal.name);
    let output_buffer = proposal.to_gzipped_bytes(app)?;
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
            abstio::list_all_objects(abstio::path_all_ltn_proposals(app.per_map.map.get_name()))
                .into_iter()
                .map(abstutil::basename)
                .collect(),
        ),
        Box::new(|name, ctx, app| {
            match Proposal::load_from_path(
                ctx,
                app,
                abstio::path_ltn_proposals(app.per_map.map.get_name(), &name),
            ) {
                Some(err_state) => Transition::Replace(err_state),
                None => preserve_state.switch_to_state(ctx, app),
            }
        }),
    )
}

// TODO Rename? This manages all proposals
pub struct AltProposals {
    // All entries are filled out, except for the current proposal being worked on
    list: Vec<Option<Proposal>>,
    current: usize,

    pub current_proposal: Proposal,
}

impl AltProposals {
    pub fn new(map: &Map, timer: &mut Timer) -> Self {
        Self {
            list: vec![None],
            current: 0,

            current_proposal: Proposal {
                map: map.get_name().clone(),
                name: "existing LTNs".to_string(),
                abst_version: map_gui::tools::version().to_string(),
                partitioning: Partitioning::seed_using_heuristics(map, timer),
                edits: Edits::default(),
            },
        }
    }

    // Special case for locking into a consultation mode
    pub fn clear_all_but_current(&mut self) {
        self.list = vec![None];
        self.current = 0;
    }

    pub fn to_widget_expanded(&self, ctx: &EventCtx, app: &App) -> Widget {
        let mut col = Vec::new();
        for (action, icon) in [
            ("New", "pencil"),
            ("Load", "folder"),
            ("Save", "save"),
            ("Share", "share"),
            ("Export GeoJSON", "export"),
        ] {
            col.push(
                ctx.style()
                    .btn_plain
                    .icon_text(&format!("system/assets/tools/{icon}.svg"), action)
                    .build_def(ctx),
            );
        }

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
                        app.per_map.alt_proposals.current_proposal.name
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
        Widget::col(col)
    }

    pub fn to_widget_collapsed(&self, ctx: &EventCtx) -> Widget {
        let mut col = Vec::new();
        for (action, icon) in [
            ("New", "pencil"),
            ("Load", "folder"),
            ("Save", "save"),
            ("Share", "share"),
            ("Export GeoJSON", "export"),
        ] {
            col.push(
                ctx.style()
                    .btn_plain
                    .icon(&format!("system/assets/tools/{icon}.svg"))
                    .build_widget(ctx, action),
            );
        }
        Widget::col(col)
    }

    pub fn handle_action(
        ctx: &mut EventCtx,
        app: &mut App,
        preserve_state: &PreserveState,
        action: &str,
    ) -> Option<Transition> {
        match action {
            "New" => {
                // TODO Hack. We want to lock people into a special base proposal. This "New"
                // button will go away entirely soon, fixing this properly
                if app.per_map.consultation_id == Some("pt2".to_string()) {
                    return None;
                }

                stash_current_proposal(app);

                // This is expensive -- maybe we should just calculate this once and keep a copy
                // forever
                ctx.loading_screen("create new proposal", |_, timer| {
                    // First undo any one-way changes. This is messy to repeat here, but it's not
                    // straightforward to use make_active.
                    let mut edits = app.per_map.map.new_edits();
                    for r in app.edits().one_ways.keys().cloned() {
                        edits.commands.push(app.per_map.map.edit_road_cmd(r, |new| {
                            *new = EditRoad::get_orig_from_osm(
                                app.per_map.map.get_r(r),
                                app.per_map.map.get_config(),
                            );
                        }));
                    }
                    app.per_map.map.must_apply_edits(edits, timer);

                    app.per_map.alt_proposals.current_proposal =
                        AltProposals::new(&app.per_map.map, timer).current_proposal;
                });

                // Start a new proposal
                app.per_map.alt_proposals.list.push(None);
                app.per_map.alt_proposals.current = app.per_map.alt_proposals.list.len() - 1;
            }
            "Load" => {
                return Some(Transition::Push(load_picker_ui(
                    ctx,
                    app,
                    preserve_state.clone(),
                )));
            }
            "Save" => {
                return Some(Transition::Push(save_ui(ctx, app, preserve_state.clone())));
            }
            "Share" => {
                return Some(Transition::Push(share::ShareProposal::new_state(ctx, app)));
            }
            "Export GeoJSON" => {
                let result = crate::export::write_geojson_file(app);
                return Some(Transition::Push(match result {
                    Ok(path) => PopupMsg::new_state(
                        ctx,
                        "LTNs exported",
                        vec![format!("Data exported to {}", path)],
                    ),
                    Err(err) => PopupMsg::new_state(ctx, "Export failed", vec![err.to_string()]),
                }));
            }
            _ => {
                if let Some(x) = action.strip_prefix("switch to proposal ") {
                    let idx = x.parse::<usize>().unwrap();
                    switch_to_existing_proposal(ctx, app, idx);
                } else if let Some(x) = action.strip_prefix("hide proposal ") {
                    let idx = x.parse::<usize>().unwrap();
                    if idx == app.per_map.alt_proposals.current {
                        // First make sure we're not hiding the current proposal
                        switch_to_existing_proposal(ctx, app, if idx == 0 { 1 } else { idx - 1 });
                    }

                    // Remove it
                    app.per_map.alt_proposals.list.remove(idx);

                    // Fix up indices
                    if idx < app.per_map.alt_proposals.current {
                        app.per_map.alt_proposals.current -= 1;
                    }
                } else {
                    return None;
                }
            }
        }

        Some(preserve_state.clone().switch_to_state(ctx, app))
    }
}

// After switching proposals, we have to recreate state
//
// To preserve per-neighborhood states, we have to transform neighbourhood IDs, which may change if
// the partitioning is different. If the boundary is a bit different, match up by all the blocks in
// the current neighbourhood.
#[derive(Clone)]
pub enum PreserveState {
    PickArea,
    Route,
    // TODO app.session.edit_mode now has state for Shortcuts...
    DesignLTN(Vec<BlockID>),
}

impl PreserveState {
    fn switch_to_state(self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self {
            PreserveState::PickArea => Transition::Replace(PickArea::new_state(ctx, app)),
            PreserveState::Route => {
                Transition::Replace(crate::route_planner::RoutePlanner::new_state(ctx, app))
            }
            PreserveState::DesignLTN(blocks) => {
                // Count which new neighbourhoods have the blocks from the original. Pick the one
                // with the most matches
                let mut count = Counter::new();
                for block in blocks {
                    count.inc(app.partitioning().block_to_neighbourhood(block));
                }

                if let EditMode::Shortcuts(ref mut maybe_focus) = app.session.edit_mode {
                    // TODO We should try to preserve the focused road at least, or the specific
                    // shortcut maybe.
                    *maybe_focus = None;
                }
                if let EditMode::FreehandFilters(_) = app.session.edit_mode {
                    app.session.edit_mode = EditMode::Filters;
                }

                Transition::Replace(crate::design_ltn::DesignLTN::new_state(
                    ctx,
                    app,
                    count.max_key(),
                ))
            }
        }
    }
}
