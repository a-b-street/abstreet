use colors::Colors;
use ezgui::UserInput;
use graphics::types::Color;
use map_model::LaneID;
use objects::ID;
use piston::input::Key;
use plugins::{Colorizer, Ctx};
use sim::{AgentID, Sim};
use std::collections::HashSet;

#[derive(PartialEq)]
pub enum ShowRouteState {
    Empty,
    Active(AgentID, HashSet<LaneID>),
}

impl ShowRouteState {
    pub fn event(&mut self, input: &mut UserInput, sim: &Sim, selected: Option<ID>) -> bool {
        if *self == ShowRouteState::Empty {
            match selected {
                Some(ID::Car(id)) => {
                    if input.key_pressed(Key::R, "show this car's route") {
                        *self = ShowRouteState::Active(AgentID::Car(id), HashSet::new());
                        return true;
                    }
                }
                Some(ID::Pedestrian(id)) => {
                    if input.key_pressed(Key::R, "show this pedestrian's route") {
                        *self = ShowRouteState::Active(AgentID::Pedestrian(id), HashSet::new());
                        return true;
                    }
                }
                _ => {}
            }
        }

        let quit = match self {
            ShowRouteState::Empty => false,
            ShowRouteState::Active(agent, ref mut lanes) => {
                if input.key_pressed(Key::Return, "stop showing route") {
                    true
                } else {
                    match sim.get_current_route(*agent) {
                        Some(route) => {
                            lanes.clear();
                            lanes.extend(route);
                            false
                        }
                        None => true,
                    }
                }
            }
        };
        if quit {
            *self = ShowRouteState::Empty;
        }
        match self {
            ShowRouteState::Empty => false,
            _ => true,
        }
    }
}

impl Colorizer for ShowRouteState {
    fn color_for(&self, obj: ID, ctx: Ctx) -> Option<Color> {
        match obj {
            ID::Lane(l) => {
                let highlight = match self {
                    ShowRouteState::Empty => false,
                    ShowRouteState::Active(_, lanes) => lanes.contains(&l),
                };
                if highlight {
                    Some(ctx.cs.get(Colors::Queued))
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}
