use colors::{ColorScheme, Colors};
use dimensioned::si;
use ezgui::GfxCtx;
use map_model::{Trace, LANE_THICKNESS};
use objects::ID;
use piston::input::Key;
use plugins::{Plugin, PluginCtx};
use sim::AgentID;
use std::f64;

pub enum ShowRouteState {
    Empty,
    Active(AgentID, Trace),
}

impl ShowRouteState {
    pub fn draw(&self, g: &mut GfxCtx, cs: &ColorScheme) {
        if let ShowRouteState::Active(_, trace) = self {
            g.draw_polygon(
                cs.get(Colors::Queued),
                &trace.get_polyline().make_polygons_blindly(LANE_THICKNESS),
            );
        }
    }
}

impl Plugin for ShowRouteState {
    fn event(&mut self, ctx: PluginCtx) -> bool {
        let (input, sim, map, selected) = (
            ctx.input,
            &ctx.primary.sim,
            &ctx.primary.map,
            ctx.primary.current_selection,
        );

        let maybe_agent = match self {
            ShowRouteState::Empty => match selected {
                Some(ID::Car(id)) => {
                    if input.key_pressed(Key::R, "show this car's route") {
                        Some(AgentID::Car(id))
                    } else {
                        None
                    }
                }
                Some(ID::Pedestrian(id)) => {
                    if input.key_pressed(Key::R, "show this pedestrian's route") {
                        Some(AgentID::Pedestrian(id))
                    } else {
                        None
                    }
                }
                _ => None,
            },
            ShowRouteState::Active(agent, _) => {
                if input.key_pressed(Key::Return, "stop showing route") {
                    None
                } else {
                    Some(*agent)
                }
            }
        };
        if let Some(agent) = maybe_agent {
            // Trace along the entire route by passing in max distance
            match sim.trace_route(agent, map, f64::MAX * si::M) {
                Some(trace) => {
                    *self = ShowRouteState::Active(agent, trace);
                }
                None => {
                    *self = ShowRouteState::Empty;
                }
            }
        } else {
            *self = ShowRouteState::Empty;
        }

        match self {
            ShowRouteState::Empty => false,
            _ => true,
        }
    }
}
