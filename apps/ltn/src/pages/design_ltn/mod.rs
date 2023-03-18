mod filters;
mod freehand_filters;
mod modals;
mod one_ways;
mod page;
mod shortcuts;
mod speed_limits;

use map_model::{IntersectionID, Road, RoadID};
use widgetry::mapspace::{ObjectID, World};
use widgetry::tools::{PolyLineLasso, PopupMsg};
use widgetry::{EventCtx, Panel};

use crate::{is_private, logic, pages, App, Neighbourhood, Transition};

pub use page::DesignLTN;

pub enum EditMode {
    Filters,
    FreehandFilters(PolyLineLasso),
    Oneways,
    // Is a road clicked on right now?
    Shortcuts(Option<shortcuts::FocusedRoad>),
    SpeedLimits,
}

pub struct EditNeighbourhood {
    // Only pub for drawing
    pub world: World<Obj>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Obj {
    Road(RoadID),
    Intersection(IntersectionID),
}
impl ObjectID for Obj {}

pub enum EditOutcome {
    Nothing,
    /// Don't recreate the Neighbourhood
    UpdatePanelAndWorld,
    /// Update the panel, world, and neighbourhood (cells and shortcuts only)
    UpdateAll,
    Transition(Transition),
}

impl EditOutcome {
    fn error(ctx: &mut EventCtx, msg: &str) -> Self {
        Self::Transition(Transition::Push(PopupMsg::new_state(
            ctx,
            "Error",
            vec![msg],
        )))
    }
}

impl EditNeighbourhood {
    pub fn temporary() -> Self {
        Self {
            world: World::new(),
        }
    }

    pub fn new(ctx: &mut EventCtx, app: &App, neighbourhood: &Neighbourhood) -> Self {
        Self {
            world: match &app.session.edit_mode {
                EditMode::Filters => filters::make_world(ctx, app, neighbourhood),
                EditMode::FreehandFilters(_) => World::new(),
                EditMode::Oneways => one_ways::make_world(ctx, app, neighbourhood),
                EditMode::Shortcuts(focus) => shortcuts::make_world(ctx, app, neighbourhood, focus),
                EditMode::SpeedLimits => speed_limits::make_world(ctx, app, neighbourhood),
            },
        }
    }

    pub fn event(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        neighbourhood: &Neighbourhood,
    ) -> EditOutcome {
        if let EditMode::FreehandFilters(_) = app.session.edit_mode {
            return freehand_filters::event(ctx, app, neighbourhood);
        }

        let outcome = self.world.event(ctx);
        let outcome = match app.session.edit_mode {
            EditMode::Filters => filters::handle_world_outcome(ctx, app, outcome),
            EditMode::FreehandFilters(_) => unreachable!(),
            EditMode::Oneways => one_ways::handle_world_outcome(ctx, app, outcome),
            EditMode::Shortcuts(_) => shortcuts::handle_world_outcome(app, outcome, neighbourhood),
            EditMode::SpeedLimits => speed_limits::handle_world_outcome(ctx, app, outcome),
        };
        if matches!(outcome, EditOutcome::Transition(_)) {
            self.world.hack_unset_hovering();
        }
        outcome
    }

    pub fn handle_panel_action(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        action: &str,
        neighbourhood: &Neighbourhood,
        panel: &mut Panel,
    ) -> EditOutcome {
        let id = neighbourhood.id;
        match action {
            "Adjust boundary" => EditOutcome::Transition(Transition::Replace(
                if let Some(custom) = app.partitioning().custom_boundaries.get(&id).cloned() {
                    pages::FreehandBoundary::edit_existing(
                        ctx,
                        app,
                        custom.name.clone(),
                        id,
                        custom,
                    )
                } else {
                    pages::SelectBoundary::new_state(ctx, app, id)
                },
            )),
            "Per-resident route impact" => EditOutcome::Transition(Transition::Replace(
                pages::PerResidentImpact::new_state(ctx, app, id, None),
            )),
            "undo" => {
                logic::map_edits::undo_proposal(ctx, app);
                // TODO Ideally, preserve panel state (checkboxes and dropdowns)
                if let EditMode::Shortcuts(ref mut maybe_focus) = app.session.edit_mode {
                    *maybe_focus = None;
                }
                if let EditMode::FreehandFilters(_) = app.session.edit_mode {
                    app.session.edit_mode = EditMode::Filters;
                }
                EditOutcome::UpdateAll
            }
            "Modal filter - no entry"
            | "Modal filter -- walking/cycling only"
            | "Bus gate"
            | "School street" => {
                app.session.edit_mode = EditMode::Filters;
                EditOutcome::UpdatePanelAndWorld
            }
            "Change modal filter" => EditOutcome::Transition(Transition::Push(
                modals::ChangeFilterType::new_state(ctx, app),
            )),
            "Freehand filters" => {
                app.session.edit_mode = EditMode::FreehandFilters(PolyLineLasso::new());
                EditOutcome::UpdatePanelAndWorld
            }
            "One-ways" => {
                app.session.edit_mode = EditMode::Oneways;
                EditOutcome::UpdatePanelAndWorld
            }
            "Shortcuts" => {
                app.session.edit_mode = EditMode::Shortcuts(None);
                EditOutcome::UpdatePanelAndWorld
            }
            "previous shortcut" => {
                if let EditMode::Shortcuts(Some(ref mut focus)) = app.session.edit_mode {
                    focus.current_idx -= 1;
                }
                // Logically we could do UpdatePanelAndWorld, but we need to be more efficient
                if let EditMode::Shortcuts(ref focus) = app.session.edit_mode {
                    let panel_piece = shortcuts::widget(ctx, app, focus.as_ref());
                    panel.replace(ctx, "edit mode contents", panel_piece);
                    self.world = shortcuts::make_world(ctx, app, neighbourhood, focus);
                }
                EditOutcome::Transition(Transition::Keep)
            }
            "next shortcut" => {
                if let EditMode::Shortcuts(Some(ref mut focus)) = app.session.edit_mode {
                    focus.current_idx += 1;
                }
                if let EditMode::Shortcuts(ref focus) = app.session.edit_mode {
                    let panel_piece = shortcuts::widget(ctx, app, focus.as_ref());
                    panel.replace(ctx, "edit mode contents", panel_piece);
                    self.world = shortcuts::make_world(ctx, app, neighbourhood, focus);
                }
                EditOutcome::Transition(Transition::Keep)
            }
            "Speed limits" => {
                app.session.edit_mode = EditMode::SpeedLimits;
                EditOutcome::UpdatePanelAndWorld
            }
            _ => EditOutcome::Nothing,
        }
    }
}

fn road_name(app: &App, road: &Road) -> String {
    let mut name = road.get_name(app.opts.language.as_ref());
    if name == "???" {
        name = "unnamed road".to_string();
    }
    if is_private(road) {
        format!("{name} (private)")
    } else {
        name
    }
}
