mod perma;
mod proposals_ui;
mod save_dialog;
mod share;

use std::collections::BTreeSet;

use anyhow::Result;

use abstutil::{Counter, Timer};
use map_model::{BuildingID, Map, MapEdits};
use widgetry::tools::PopupMsg;
use widgetry::{EventCtx, State};

use crate::logic::{BlockID, Partitioning};
use crate::{pages, App, Transition};

pub use share::PROPOSAL_HOST_URL;

pub struct Proposals {
    // Note MapEdits for the current proposal is NOT up-to-date; map.get_edits() is
    // TODO Or we could just copy here in apply_edits; would that be simpler?
    pub list: Vec<Proposal>,
    pub current: usize,
}

/// Captures all of the edits somebody makes to a map in the LTN tool.
/// TODO This should just be MapEdits, but we need to deal with Partitioning still
/// Note "existing LTNs" is a special reserved name
#[derive(Clone)]
pub struct Proposal {
    pub partitioning: Partitioning,
    pub edits: MapEdits,

    /// If this proposal is an edit to another proposal, store its name
    unsaved_parent: Option<String>,
}

impl Proposal {
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
        }

        app.apply_edits(proposal.edits.clone());
        crate::redraw_all_filters(ctx, app);

        app.per_map.proposals.list.push(proposal);
        app.per_map.proposals.current = app.per_map.proposals.list.len() - 1;

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
    // We need to sync MapEdits here!
    app.per_map.proposals.list[app.per_map.proposals.current].edits =
        app.per_map.map.get_edits().clone();
    // TODO And compress?
}

impl Proposals {
    // This calculates partitioning, which is expensive
    pub fn new(map: &Map, timer: &mut Timer) -> Self {
        Self {
            list: vec![Proposal {
                partitioning: Partitioning::seed_using_heuristics(map, timer),
                edits: map.get_edits().clone(),
                unsaved_parent: None,
            }],
            current: 0,
        }
    }

    pub fn get_current(&self) -> &Proposal {
        &self.list[self.current]
    }
    pub fn mut_current(&mut self) -> &mut Proposal {
        &mut self.list[self.current]
    }

    // Special case for locking into a consultation mode
    pub fn force_current_to_basemap(&mut self) {
        let mut current = self.list.remove(self.current);
        current.edits.edits_name = "existing LTNs".to_string();
        self.list = vec![current];
        self.current = 0;
    }

    /// Call before making any changes to fork a copy of the proposal
    pub fn before_edit(&mut self, edits: &mut MapEdits) {
        // Fork the proposal or not?
        if self.get_current().unsaved_parent.is_none() {
            // Fork a new proposal if we're starting from the immutable baseline
            let from_immutable = self.current == 0;
            if from_immutable {
                // We don't need to to sync MapEdits before doing this; this is the immutable,
                // original edits
                self.list.insert(1, self.list[0].clone());
                self.current = 1;
            }
            // Otherwise, just replace the current proposal with something that's clearly edited
            self.list[self.current].unsaved_parent = Some(edits.edits_name.clone());

            if from_immutable {
                // There'll be name collision if people start multiple unsaved files, but it
                // shouldn't cause problems
                edits.edits_name = "new proposal*".to_string();
            } else {
                edits.edits_name = format!("{}*", self.get_current().edits.edits_name);
            }
        }
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
    Census,
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
            PreserveState::Census => Transition::Replace(pages::Census::new_state(ctx, app)),
        }
    }
}
