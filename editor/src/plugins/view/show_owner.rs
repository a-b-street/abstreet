use ezgui::Color;
use map_model::{BuildingID, RoadID};
use objects::{Ctx, ID};
use plugins::{Plugin, PluginCtx};
use render::ExtraShapeID;
use sim::CarID;
use std::collections::HashSet;

// TODO rename ShowAssociated?
pub enum ShowOwnerState {
    Inactive,
    BuildingSelected(BuildingID, HashSet<CarID>),
    CarSelected(CarID, Option<BuildingID>),
    ShapeSelected(ExtraShapeID, Option<(RoadID, bool)>),
}

impl ShowOwnerState {
    pub fn new() -> ShowOwnerState {
        ShowOwnerState::Inactive
    }
}

impl Plugin for ShowOwnerState {
    fn ambient_event(&mut self, ctx: &mut PluginCtx) {
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
            ShowOwnerState::ShapeSelected(es, _) => {
                reset = selected != Some(ID::ExtraShape(*es));
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
                Some(ID::ExtraShape(id)) => {
                    new_state = Some(ShowOwnerState::ShapeSelected(
                        id,
                        ctx.primary.draw_map.get_es(id).road,
                    ));
                }
                _ => {}
            },
            _ => {}
        }
        if let Some(s) = new_state {
            *self = s;
        }
    }

    fn new_color_for(&self, obj: ID, ctx: &mut Ctx) -> Option<Color> {
        let color = ctx.cs.get("car/building owner", Color::PURPLE);
        match (self, obj) {
            (ShowOwnerState::BuildingSelected(_, cars), ID::Car(id)) => {
                if cars.contains(&id) {
                    return Some(color);
                }
            }
            (ShowOwnerState::CarSelected(_, Some(id1)), ID::Building(id2)) => {
                if *id1 == id2 {
                    return Some(color);
                }
            }
            (ShowOwnerState::ShapeSelected(_, Some((r, fwds))), ID::Lane(l)) => {
                let parent = ctx.map.get_parent(l);
                if parent.id == *r
                    && ((*fwds && parent.is_forwards(l)) || (!fwds && parent.is_backwards(l)))
                {
                    return Some(color);
                }
            }
            _ => {}
        }
        None
    }
}
