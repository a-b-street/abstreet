use crate::helpers::ID;
use crate::render::ExtraShapeID;
use crate::ui::UI;
use ezgui::Color;
use map_model::{BuildingID, DirectedRoadID, IntersectionID};
use sim::{AgentID, CarID};
use std::collections::{HashMap, HashSet};

// TODO Maybe just store the IDs upfront.
// TODO ShapeSelected only makes sense in DebugMode (more generally, extra shapes only make sense
// there...). The rest only make sense in sandbox/AB test mode.
pub enum ShowAssociatedState {
    Inactive,
    BuildingSelected(BuildingID, HashSet<CarID>),
    CarSelected(CarID, Option<BuildingID>),
    ShapeSelected(ExtraShapeID, Option<DirectedRoadID>),
    IntersectionSelected(IntersectionID, HashSet<AgentID>),
}

impl ShowAssociatedState {
    pub fn event(&mut self, ui: &UI) {
        let selected = ui.primary.current_selection.clone();
        let sim = &ui.primary.sim;

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
                // TODO Only if the time has changed, actually.
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
                            .map(|p| p.vehicle.id)
                            .collect(),
                    );
                }
                Some(ID::Car(id)) => {
                    *self = ShowAssociatedState::CarSelected(id, sim.get_owner_of_car(id));
                }
                Some(ID::ExtraShape(id)) => {
                    *self =
                        ShowAssociatedState::ShapeSelected(id, ui.primary.draw_map.get_es(id).road);
                }
                Some(ID::Intersection(id)) => {
                    *self =
                        ShowAssociatedState::IntersectionSelected(id, sim.get_accepted_agents(id));
                }
                _ => {}
            }
        }
    }

    pub fn override_colors(&self, colors: &mut HashMap<ID, Color>, ui: &UI) {
        let color = ui
            .cs
            .get_def("something associated with something else", Color::PURPLE);
        match self {
            ShowAssociatedState::BuildingSelected(_, cars) => {
                for c in cars {
                    colors.insert(ID::Car(*c), color);
                }
            }
            ShowAssociatedState::CarSelected(_, Some(b)) => {
                colors.insert(ID::Building(*b), color);
            }
            ShowAssociatedState::ShapeSelected(_, Some(dr)) => {
                let r = ui.primary.map.get_r(dr.id);
                if dr.forwards {
                    for (l, _) in &r.children_forwards {
                        colors.insert(ID::Lane(*l), color);
                    }
                } else {
                    for (l, _) in &r.children_backwards {
                        colors.insert(ID::Lane(*l), color);
                    }
                }
            }
            ShowAssociatedState::IntersectionSelected(_, agents) => {
                for a in agents {
                    colors.insert(ID::from_agent(*a), color);
                }
            }
            _ => {}
        }
    }
}
