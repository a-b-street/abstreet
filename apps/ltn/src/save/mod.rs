mod perma;
mod share;

use std::collections::BTreeSet;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use abstio::MapName;
use abstutil::{Counter, Timer};
use map_model::{BuildingID, EditRoad, Map};
use widgetry::tools::{ChooseSomething, PopupMsg};
use widgetry::{
    lctrl, Choice, DrawBaselayer, EventCtx, GfxCtx, Key, Line, MultiKey, Outcome, Panel, State,
    TextBox, Widget,
};

use crate::logic::{BlockID, Partitioning};
use crate::{pages, App, Edits, Transition};

pub use share::PROPOSAL_HOST_URL;

/// Captures all of the edits somebody makes to a map in the LTN tool. Note this is separate from
/// `map_model::MapEdits`.
#[derive(Clone, Serialize, Deserialize)]
pub struct Proposal {
    pub map: MapName,
    /// "existing LTNs" is a special reserved name
    pub name: String,
    pub abst_version: String,

    pub partitioning: Partitioning,
    pub edits: Edits,

    /// If this proposal is an edit to another proposal, store its name
    #[serde(skip_serializing, skip_deserializing)]
    unsaved_parent: Option<String>,
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

        app.per_map.proposals.current_proposal = self;
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
            app.per_map.proposals.list.push(None);
            app.per_map.proposals.current = app.per_map.proposals.list.len() - 1;
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
        .proposals
        .list
        .get_mut(app.per_map.proposals.current)
        .unwrap() = Some(app.per_map.proposals.current_proposal.clone());
}

fn switch_to_existing_proposal(ctx: &mut EventCtx, app: &mut App, idx: usize) {
    stash_current_proposal(app);

    let proposal = app
        .per_map
        .proposals
        .list
        .get_mut(idx)
        .unwrap()
        .take()
        .unwrap();
    app.per_map.proposals.current = idx;

    proposal.make_active(ctx, app);
}

struct SaveDialog {
    panel: Panel,
    preserve_state: PreserveState,
    can_overwrite: bool,
}

impl SaveDialog {
    fn new_state(
        ctx: &mut EventCtx,
        app: &App,
        preserve_state: PreserveState,
    ) -> Box<dyn State<App>> {
        let parent = app
            .per_map
            .proposals
            .current_proposal
            .unsaved_parent
            .clone();
        let can_overwrite = parent.is_some() && parent != Some("existing LTNs".to_string());

        let mut state = Self {
            panel: Panel::new_builder(Widget::col(vec![
                Widget::row(vec![
                    Line("Save proposal").small_heading().into_widget(ctx),
                    ctx.style().btn_close_widget(ctx),
                ]),
                if can_overwrite {
                    Widget::row(vec![
                        ctx.style()
                            .btn_solid_destructive
                            .text(format!("Overwrite \"{}\"", parent.unwrap()))
                            .build_widget(ctx, "Overwrite"),
                        Line("Or save a new copy below")
                            .secondary()
                            .into_widget(ctx)
                            .centered_vert(),
                    ])
                } else {
                    Widget::nothing()
                },
                Widget::row(vec![
                    TextBox::default_widget(ctx, "input", String::new()),
                    Widget::placeholder(ctx, "Save as"),
                ]),
                Widget::placeholder(ctx, "warning"),
            ]))
            .build(ctx),
            preserve_state,
            can_overwrite,
        };
        state.name_updated(ctx);
        Box::new(state)
    }

    fn name_updated(&mut self, ctx: &mut EventCtx) {
        let name = self.panel.text_box("input");

        let warning = if name == "existing LTNs" {
            Some("You can't overwrite the name \"existing LTNs\"")
        } else if name.is_empty() {
            Some("You have to name this proposal")
        } else {
            None
        };

        let btn = ctx
            .style()
            .btn_solid_primary
            .text("Save as")
            .disabled(warning.is_some())
            .hotkey(if self.can_overwrite {
                None
            } else {
                Some(MultiKey::from(Key::Enter))
            })
            .build_def(ctx);
        self.panel.replace(ctx, "Save as", btn);

        if let Some(warning) = warning {
            self.panel
                .replace(ctx, "warning", Line(warning).into_widget(ctx));
        } else {
            self.panel
                .replace(ctx, "warning", Widget::placeholder(ctx, "warning"));
        }
    }

    fn error(&self, ctx: &mut EventCtx, app: &mut App, err: impl AsRef<str>) -> Transition {
        Transition::Multi(vec![
            self.preserve_state.switch_to_state(ctx, app),
            Transition::Push(PopupMsg::new_state(ctx, "Error", vec![err])),
        ])
    }
}

impl State<App> for SaveDialog {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => Transition::Pop,
                "Save as" => {
                    let name = self.panel.text_box("input");

                    // TODO If we're clobbering something that exists in Proposals especially...
                    // watch out

                    app.per_map.proposals.current_proposal.name = name;
                    app.per_map.proposals.current_proposal.unsaved_parent = None;
                    return match inner_save(app) {
                        // If we changed the name, we'll want to recreate the panel
                        Ok(()) => self.preserve_state.switch_to_state(ctx, app),
                        Err(err) => {
                            self.error(ctx, app, format!("Couldn't save proposal: {}", err))
                        }
                    };
                }
                "Overwrite" => {
                    // TODO If the user loaded the parent file again, this'll be confusing. Maybe
                    // ban that?

                    let proposals = &mut app.per_map.proposals;
                    proposals.current_proposal.name =
                        proposals.current_proposal.unsaved_parent.take().unwrap();

                    return match inner_save(app) {
                        Ok(()) => self.preserve_state.switch_to_state(ctx, app),
                        // TODO If we fail to save for some reason, the Proposals panel gets out
                        // of sync with the filesystem
                        Err(err) => {
                            self.error(ctx, app, format!("Couldn't save proposal: {}", err))
                        }
                    };
                }
                _ => unreachable!(),
            },
            Outcome::Changed(_) => {
                self.name_updated(ctx);
                Transition::Keep
            }
            _ => {
                if ctx.normal_left_click() && ctx.canvas.get_cursor_in_screen_space().is_none() {
                    return Transition::Pop;
                }
                Transition::Keep
            }
        }
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::PreviousState
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        map_gui::tools::grey_out_map(g, app);
        self.panel.draw(g);
    }
}

fn inner_save(app: &App) -> Result<()> {
    let proposal = &app.per_map.proposals.current_proposal;
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
        Box::new(move |name, ctx, app| {
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

pub struct Proposals {
    // All entries are filled out, except for the current proposal being worked on
    list: Vec<Option<Proposal>>,
    current: usize,

    pub current_proposal: Proposal,
}

impl Proposals {
    // This calculates partitioning, which is expensive
    pub fn new(map: &Map, edits: Edits, timer: &mut Timer) -> Self {
        Self {
            list: vec![None],
            current: 0,

            current_proposal: Proposal {
                map: map.get_name().clone(),
                name: "existing LTNs".to_string(),
                abst_version: map_gui::tools::version().to_string(),
                partitioning: Partitioning::seed_using_heuristics(map, timer),
                edits,
                unsaved_parent: None,
            },
        }
    }

    // Special case for locking into a consultation mode
    pub fn clear_all_but_current(&mut self) {
        self.list = vec![None];
        self.current = 0;
    }

    /// Call before making any changes to fork a copy of the proposal and to preserve edit history
    pub fn before_edit(&mut self) {
        // Fork the proposal or not?
        if self.current_proposal.unsaved_parent.is_none() {
            // Fork a new proposal if we're starting from the immutable baseline
            let from_immutable = self.current == 0;
            if from_immutable {
                self.list
                    .insert(self.current, Some(self.current_proposal.clone()));
                self.current += 1;
                assert!(self.list[self.current].is_none());
            }
            // Otherwise, just replace the current proposal with something that's clearly edited
            self.current_proposal.unsaved_parent = Some(self.current_proposal.name.clone());
            if from_immutable {
                // There'll be name collision if people start multiple unsaved files, but it
                // shouldn't cause problems
                self.current_proposal.name = "new proposal*".to_string();
            } else {
                self.current_proposal.name = format!("{}*", self.current_proposal.name);
            }
        }

        // Handle undo history
        let copy = self.current_proposal.edits.clone();
        self.current_proposal.edits.previous_version = Box::new(Some(copy));
    }

    /// If it's possible no edits were made, undo the previous call to `before_edit` and collapse
    /// the redundant piece of history. Returns true if the edit was indeed empty.
    pub fn cancel_empty_edit(&mut self) -> bool {
        if let Some(prev) = self.current_proposal.edits.previous_version.take() {
            if self.current_proposal.edits.roads == prev.roads
                && self.current_proposal.edits.intersections == prev.intersections
                && self.current_proposal.edits.one_ways == prev.one_ways
            {
                self.current_proposal.edits.previous_version = prev.previous_version;

                // TODO Maybe "unfork" the proposal -- remove the unsaved marker. But that depends
                // on if the previous proposal was already modified or not.

                return true;
            } else {
                // There was a real difference, keep
                self.current_proposal.edits.previous_version = Box::new(Some(prev));
            }
        }
        false
    }

    pub fn to_widget_expanded(&self, ctx: &EventCtx, app: &App) -> Widget {
        let mut col = Vec::new();
        for (action, icon, hotkey) in [
            ("New", "pencil", None),
            ("Load", "folder", None),
            ("Save", "save", Some(MultiKey::from(lctrl(Key::S)))),
            ("Share", "share", None),
            ("Export GeoJSON", "export", None),
        ] {
            col.push(
                ctx.style()
                    .btn_plain
                    .icon_text(&format!("system/assets/tools/{icon}.svg"), action)
                    .hotkey(hotkey)
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
                        app.per_map.proposals.current_proposal.name
                    ))
                    .disabled(true)
                    .build_def(ctx)
            };
            col.push(Widget::row(vec![
                button,
                // The first proposal (usually "existing LTNs", unless we're in a special consultation
                // mode) is special and can't ever be removed
                if idx != 0 {
                    ctx.style()
                        .btn_close()
                        .disabled(self.list.len() == 1)
                        .build_widget(ctx, &format!("hide proposal {}", idx))
                } else {
                    Widget::nothing()
                },
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
                // Fork a new proposal from the first one
                if app.per_map.proposals.current != 0 {
                    switch_to_existing_proposal(ctx, app, 0);
                }

                app.per_map.proposals.before_edit();
            }
            "Load" => {
                return Some(Transition::Push(load_picker_ui(
                    ctx,
                    app,
                    preserve_state.clone(),
                )));
            }
            "Save" => {
                return Some(Transition::Push(SaveDialog::new_state(
                    ctx,
                    app,
                    preserve_state.clone(),
                )));
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
                    if idx == app.per_map.proposals.current {
                        // First make sure we're not hiding the current proposal
                        switch_to_existing_proposal(ctx, app, if idx == 0 { 1 } else { idx - 1 });
                    }

                    // Remove it
                    app.per_map.proposals.list.remove(idx);

                    // Fix up indices
                    if idx < app.per_map.proposals.current {
                        app.per_map.proposals.current -= 1;
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
    Crossings,
    // TODO app.session.edit_mode now has state for Shortcuts...
    DesignLTN(BTreeSet<BlockID>),
    PerResidentImpact(BTreeSet<BlockID>, Option<BuildingID>),
    CycleNetwork,
}

impl PreserveState {
    fn switch_to_state(&self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self {
            PreserveState::PickArea => Transition::Replace(pages::PickArea::new_state(ctx, app)),
            PreserveState::Route => Transition::Replace(pages::RoutePlanner::new_state(ctx, app)),
            PreserveState::Crossings => Transition::Replace(pages::Crossings::new_state(ctx, app)),
            PreserveState::DesignLTN(blocks) => {
                // Count which new neighbourhoods have the blocks from the original. Pick the one
                // with the most matches
                let mut count = Counter::new();
                for block in blocks {
                    count.inc(app.partitioning().block_to_neighbourhood(*block));
                }

                if let pages::EditMode::Shortcuts(ref mut maybe_focus) = app.session.edit_mode {
                    // TODO We should try to preserve the focused road at least, or the specific
                    // shortcut maybe.
                    *maybe_focus = None;
                }
                if let pages::EditMode::FreehandFilters(_) = app.session.edit_mode {
                    app.session.edit_mode = pages::EditMode::Filters;
                }

                Transition::Replace(pages::DesignLTN::new_state(ctx, app, count.max_key()))
            }
            PreserveState::PerResidentImpact(blocks, current_target) => {
                let mut count = Counter::new();
                for block in blocks {
                    count.inc(app.partitioning().block_to_neighbourhood(*block));
                }
                Transition::Replace(pages::PerResidentImpact::new_state(
                    ctx,
                    app,
                    count.max_key(),
                    *current_target,
                ))
            }
            PreserveState::CycleNetwork => {
                Transition::Replace(pages::CycleNetwork::new_state(ctx, app))
            }
        }
    }
}
