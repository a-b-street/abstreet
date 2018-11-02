use dimensioned::si;
use ezgui::{Color, GfxCtx};
use map_model::{Trace, LANE_THICKNESS};
use objects::Ctx;
use piston::input::Key;
use plugins::{Plugin, PluginCtx};
use sim::{Tick, TripID};
use std::f64;

pub enum ShowRouteState {
    Inactive,
    Active(Tick, TripID, Option<Trace>),
}

impl ShowRouteState {
    pub fn new() -> ShowRouteState {
        ShowRouteState::Inactive
    }
}

impl Plugin for ShowRouteState {
    fn event(&mut self, ctx: PluginCtx) -> bool {
        let mut new_state: Option<ShowRouteState> = None;

        match self {
            ShowRouteState::Inactive => {
                if let Some(trip) = ctx
                    .primary
                    .current_selection
                    .and_then(|id| id.agent_id())
                    .and_then(|agent| ctx.primary.sim.agent_to_trip(agent))
                {
                    if ctx
                        .input
                        .key_pressed(Key::R, &format!("show {}'s route", trip))
                    {
                        new_state = Some(show_route(trip, ctx));
                    }
                };
            }
            ShowRouteState::Active(time, trip, _) => {
                if ctx.input.key_pressed(Key::Return, "stop showing route") {
                    new_state = Some(ShowRouteState::Inactive);
                } else if *time != ctx.primary.sim.time {
                    new_state = Some(show_route(*trip, ctx));
                }
            }
        };
        if let Some(s) = new_state {
            *self = s;
        }

        match self {
            ShowRouteState::Inactive => false,
            _ => true,
        }
    }

    fn draw(&self, g: &mut GfxCtx, ctx: Ctx) {
        if let ShowRouteState::Active(_, _, Some(trace)) = self {
            g.draw_polygon(
                ctx.cs.get("route", Color::rgba(255, 0, 0, 0.8)),
                &trace.get_polyline().make_polygons_blindly(LANE_THICKNESS),
            );
        }
    }
}

fn show_route(trip: TripID, ctx: PluginCtx) -> ShowRouteState {
    let time = ctx.primary.sim.time;
    if let Some(agent) = ctx.primary.sim.trip_to_agent(trip) {
        // Trace along the entire route by passing in max distance
        if let Some(trace) = ctx
            .primary
            .sim
            .trace_route(agent, &ctx.primary.map, f64::MAX * si::M)
        {
            ShowRouteState::Active(time, trip, Some(trace))
        } else {
            warn!("{} has no trace right now", agent);
            ShowRouteState::Active(time, trip, None)
        }
    } else {
        warn!(
            "{} has no agent associated right now; is the trip done?",
            trip
        );
        ShowRouteState::Active(time, trip, None)
    }
}
