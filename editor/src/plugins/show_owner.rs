use colors::Colors;
use ezgui::Color;
use map_model::BuildingID;
use objects::{Ctx, ID};
use plugins::{Colorizer, PluginCtx};
use sim::CarID;
use std::collections::HashSet;

pub enum ShowOwnerState {
    Inactive,
    BuildingSelected(BuildingID, HashSet<CarID>),
    CarSelected(CarID, Option<BuildingID>),
}

impl ShowOwnerState {
    pub fn new() -> ShowOwnerState {
        ShowOwnerState::Inactive
    }
}

impl Colorizer for ShowOwnerState {
    fn event(&mut self, ctx: PluginCtx) -> bool {
        let (selected, sim) = (ctx.primary.current_selection, &ctx.primary.sim);

        // Reset to Inactive when appropriate
        let mut reset = false;
        match self {
            ShowOwnerState::Inactive => {}
            ShowOwnerState::BuildingSelected(b, _) => {
                reset = selected != Some(ID::Building(*b));
            }
            ShowOwnerState::CarSelected(c, _) => {
                reset = selected != Some(ID::Car(*c));
            }
        }
        if reset {
            *self = ShowOwnerState::Inactive;
        }

        let mut new_state: Option<ShowOwnerState> = None;
        match self {
            ShowOwnerState::Inactive => match selected {
                Some(ID::Building(id)) => {
                    new_state = Some(ShowOwnerState::BuildingSelected(
                        id,
                        sim.get_parked_cars_by_owner(id)
                            .iter()
                            .map(|p| p.car)
                            .collect(),
                    ));
                }
                Some(ID::Car(id)) => {
                    new_state = Some(ShowOwnerState::CarSelected(id, sim.get_owner_of_car(id)));
                }
                _ => {}
            },
            _ => {}
        }
        if let Some(s) = new_state {
            *self = s;
        }
        // TODO This is a weird exception -- this plugin doesn't consume input, so never treat it
        // as active for blocking input
        false
    }

    fn color_for(&self, obj: ID, ctx: Ctx) -> Option<Color> {
        match (self, obj) {
            (ShowOwnerState::BuildingSelected(_, cars), ID::Car(id)) => {
                if cars.contains(&id) {
                    // TODO really got lazy defining colors
                    return Some(ctx.cs.get(Colors::SearchResult));
                }
            }
            (ShowOwnerState::CarSelected(_, Some(id1)), ID::Building(id2)) => {
                if *id1 == id2 {
                    return Some(ctx.cs.get(Colors::SearchResult));
                }
            }
            _ => {}
        }
        None
    }
}
