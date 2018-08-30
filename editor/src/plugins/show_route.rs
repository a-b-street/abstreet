use colors::{ColorScheme, Colors};
use ezgui::input::UserInput;
use graphics::types::Color;
use map_model::LaneID;
use piston::input::Key;
use sim::{AgentID, Sim};
use std::collections::HashSet;

pub enum ShowRouteState {
    Empty,
    Active(AgentID, HashSet<LaneID>),
}

impl ShowRouteState {
    pub fn event(&mut self, input: &mut UserInput, sim: &Sim) -> bool {
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
        quit
    }

    pub fn color_l(&self, l: LaneID, cs: &ColorScheme) -> Option<Color> {
        let highlight = match self {
            ShowRouteState::Empty => false,
            ShowRouteState::Active(_, lanes) => lanes.contains(&l),
        };
        if highlight {
            Some(cs.get(Colors::Queued))
        } else {
            None
        }
    }
}
