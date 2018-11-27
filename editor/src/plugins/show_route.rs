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
    DebugAllRoutes(Tick, Vec<Trace>),
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
                    .map(|agent| ctx.primary.sim.agent_to_trip(agent))
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
                } else if ctx
                    .input
                    .key_pressed(Key::A, "show routes for all trips, to debug")
                {
                    new_state = Some(debug_all_routes(ctx));
                } else if *time != ctx.primary.sim.time {
                    new_state = Some(show_route(*trip, ctx));
                }
            }
            ShowRouteState::DebugAllRoutes(time, _) => {
                if ctx
                    .input
                    .key_pressed(Key::Return, "stop showing all routes")
                {
                    new_state = Some(ShowRouteState::Inactive);
                } else if *time != ctx.primary.sim.time {
                    new_state = Some(debug_all_routes(ctx));
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
        match self {
            ShowRouteState::Active(_, _, Some(trace)) => {
                g.draw_polygon(
                    ctx.cs.get("route", Color::rgba(255, 0, 0, 0.8)),
                    &trace.make_polygons_blindly(LANE_THICKNESS),
                );
            }
            ShowRouteState::DebugAllRoutes(_, traces) => {
                for t in traces {
                    g.draw_polygon(
                        ctx.cs.get("route", Color::rgba(255, 0, 0, 0.8)),
                        &t.make_polygons_blindly(LANE_THICKNESS),
                    );
                }
            }
            _ => {}
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

fn debug_all_routes(ctx: PluginCtx) -> ShowRouteState {
    let sim = &ctx.primary.sim;
    let mut traces: Vec<Trace> = Vec::new();
    for trip in sim.get_stats().canonical_pt_per_trip.keys() {
        if let Some(agent) = sim.trip_to_agent(*trip) {
            if let Some(trace) = sim.trace_route(agent, &ctx.primary.map, f64::MAX * si::M) {
                traces.push(trace);
            }
        }
    }
    ShowRouteState::DebugAllRoutes(sim.time, traces)
}
