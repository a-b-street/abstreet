use dimensioned::si;
use ezgui::{Color, GfxCtx};
use map_model::{Trace, LANE_THICKNESS};
use objects::Ctx;
use piston::input::Key;
use plugins::{Plugin, PluginCtx};
use sim::TripID;
use std::f64;

pub enum ShowRouteState {
    Empty,
    Active(TripID, Trace),
}

impl Plugin for ShowRouteState {
    fn event(&mut self, ctx: PluginCtx) -> bool {
        let maybe_trip = match self {
            ShowRouteState::Empty => ctx
                .primary
                .current_selection
                .and_then(|id| id.agent_id())
                .and_then(|agent| ctx.primary.sim.agent_to_trip(agent))
                .and_then(|trip| {
                    if ctx
                        .input
                        .key_pressed(Key::R, &format!("show {}'s route", trip))
                    {
                        Some(trip)
                    } else {
                        None
                    }
                }),
            ShowRouteState::Active(trip, _) => {
                if ctx.input.key_pressed(Key::Return, "stop showing route") {
                    None
                } else {
                    Some(*trip)
                }
            }
        };
        if let Some(trip) = maybe_trip {
            if let Some(agent) = ctx.primary.sim.trip_to_agent(trip) {
                // Trace along the entire route by passing in max distance
                if let Some(trace) =
                    ctx.primary
                        .sim
                        .trace_route(agent, &ctx.primary.map, f64::MAX * si::M)
                {
                    *self = ShowRouteState::Active(trip, trace);
                } else {
                    warn!("{} has no trace right now", agent);
                }
            } else {
                warn!(
                    "{} has no agent associated right now; is the trip done?",
                    trip
                );
            }
        } else {
            *self = ShowRouteState::Empty;
        }

        match self {
            ShowRouteState::Empty => false,
            _ => true,
        }
    }

    fn draw(&self, g: &mut GfxCtx, ctx: Ctx) {
        if let ShowRouteState::Active(_, trace) = self {
            g.draw_polygon(
                ctx.cs.get("route", Color::rgba(255, 0, 0, 0.8)),
                &trace.get_polyline().make_polygons_blindly(LANE_THICKNESS),
            );
        }
    }
}
