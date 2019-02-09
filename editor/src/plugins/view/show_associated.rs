use crate::objects::{DrawCtx, ID};
use crate::plugins::{AmbientPlugin, PluginCtx};
use crate::render::ExtraShapeID;
use ezgui::Color;
use map_model::{BuildingID, IntersectionID, RoadID};
use sim::{AgentID, CarID};
use std::collections::HashSet;

pub enum ShowAssociatedState {
    Inactive,
    BuildingSelected(BuildingID, HashSet<CarID>),
    CarSelected(CarID, Option<BuildingID>),
    ShapeSelected(ExtraShapeID, Option<(RoadID, bool)>),
    IntersectionSelected(IntersectionID, HashSet<AgentID>),
}

impl ShowAssociatedState {
    pub fn new() -> ShowAssociatedState {
        ShowAssociatedState::Inactive
    }
}

impl AmbientPlugin for ShowAssociatedState {
    fn ambient_event(&mut self, ctx: &mut PluginCtx) {
        let (selected, sim) = (ctx.primary.current_selection, &ctx.primary.sim);

        // Reset to Inactive when appropriate
        let mut reset = false;
        match self {
            ShowAssociatedState::Inactive => {}
            ShowAssociatedState::BuildingSelected(b, _) => {
                reset = selected != Some(ID::Building(*b));
            }
            ShowAssociatedState::CarSelected(c, _) => {
                reset = selected != Some(ID::Car(*c));
            }
            ShowAssociatedState::ShapeSelected(es, _) => {
                reset = selected != Some(ID::ExtraShape(*es));
            }
            ShowAssociatedState::IntersectionSelected(_, _) => {
                // Always recalculate.
                // TODO Only if the tick has changed, actually.
                reset = true;
            }
        }
        if reset {
            *self = ShowAssociatedState::Inactive;
        }

        if let ShowAssociatedState::Inactive = self {
            match selected {
                Some(ID::Building(id)) => {
                    *self = ShowAssociatedState::BuildingSelected(
                        id,
                        sim.get_parked_cars_by_owner(id)
                            .iter()
                            .map(|p| p.car)
                            .collect(),
                    );
                }
                Some(ID::Car(id)) => {
                    *self = ShowAssociatedState::CarSelected(id, sim.get_owner_of_car(id));
                }
                Some(ID::ExtraShape(id)) => {
                    *self = ShowAssociatedState::ShapeSelected(
                        id,
                        ctx.primary.draw_map.get_es(id).road,
                    );
                }
                Some(ID::Intersection(id)) => {
                    *self =
                        ShowAssociatedState::IntersectionSelected(id, sim.get_accepted_agents(id));
                }
                _ => {}
            };
        }
    }

    fn color_for(&self, obj: ID, ctx: &DrawCtx) -> Option<Color> {
        let color = ctx
            .cs
            .get_def("something associated with something else", Color::PURPLE);
        match (self, obj) {
            (ShowAssociatedState::BuildingSelected(_, cars), ID::Car(id)) => {
                if cars.contains(&id) {
                    return Some(color);
                }
            }
            (ShowAssociatedState::CarSelected(_, Some(id1)), ID::Building(id2)) => {
                if *id1 == id2 {
                    return Some(color);
                }
            }
            (ShowAssociatedState::ShapeSelected(_, Some((r, fwds))), ID::Lane(l)) => {
                let parent = ctx.map.get_parent(l);
                if parent.id == *r
                    && ((*fwds && parent.is_forwards(l)) || (!fwds && parent.is_backwards(l)))
                {
                    return Some(color);
                }
            }
            (ShowAssociatedState::IntersectionSelected(_, agents), _) => {
                if let Some(agent) = obj.agent_id() {
                    if agents.contains(&agent) {
                        return Some(color);
                    }
                }
            }
            _ => {}
        }
        None
    }
}
