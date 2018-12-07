use crate::objects::Ctx;
use crate::plugins::{Plugin, PluginCtx};
use dimensioned::si;
use ezgui::{Color, GfxCtx};
use map_model::{Trace, LANE_THICKNESS};
use piston::input::Key;
use sim::{Tick, TripID};
use std::f64;

pub struct ShowRouteState {
    state: State,
    toggle_key: Key,
    all_key: Key,
}

enum State {
    Inactive,
    Active(Tick, TripID, Option<Trace>),
    DebugAllRoutes(Tick, Vec<Trace>),
}

impl ShowRouteState {
    pub fn new(toggle_key: Key, all_key: Key) -> ShowRouteState {
        ShowRouteState {
            state: State::Inactive,
            toggle_key,
            all_key,
        }
    }
}

impl Plugin for ShowRouteState {
    fn ambient_event(&mut self, ctx: &mut PluginCtx) {
        match self.state {
            State::Inactive => {
                if let Some(agent) = ctx.primary.current_selection.and_then(|id| id.agent_id()) {
                    if let Some(trip) = ctx.primary.sim.agent_to_trip(agent) {
                        if ctx
                            .input
                            .key_pressed(self.toggle_key, &format!("show {}'s route", agent))
                        {
                            self.state = show_route(trip, ctx);
                        }
                    }
                };
            }
            State::Active(time, trip, _) => {
                if ctx.input.key_pressed(self.toggle_key, "stop showing route") {
                    self.state = State::Inactive;
                } else if ctx
                    .input
                    .key_pressed(self.all_key, "show routes for all trips, to debug")
                {
                    self.state = debug_all_routes(ctx);
                } else if time != ctx.primary.sim.time {
                    self.state = show_route(trip, ctx);
                }
            }
            State::DebugAllRoutes(time, _) => {
                if ctx
                    .input
                    .key_pressed(self.all_key, "stop showing all routes")
                {
                    self.state = State::Inactive;
                } else if time != ctx.primary.sim.time {
                    self.state = debug_all_routes(ctx);
                }
            }
        };
    }

    fn draw(&self, g: &mut GfxCtx, ctx: &mut Ctx) {
        match &self.state {
            State::Active(_, _, Some(trace)) => {
                g.draw_polygon(
                    ctx.cs.get("route", Color::rgba(255, 0, 0, 0.8)),
                    &trace.make_polygons_blindly(LANE_THICKNESS),
                );
            }
            State::DebugAllRoutes(_, traces) => {
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

fn show_route(trip: TripID, ctx: &mut PluginCtx) -> State {
    let time = ctx.primary.sim.time;
    if let Some(agent) = ctx.primary.sim.trip_to_agent(trip) {
        // Trace along the entire route by passing in max distance
        if let Some(trace) = ctx
            .primary
            .sim
            .trace_route(agent, &ctx.primary.map, f64::MAX * si::M)
        {
            State::Active(time, trip, Some(trace))
        } else {
            warn!("{} has no trace right now", agent);
            State::Active(time, trip, None)
        }
    } else {
        warn!(
            "{} has no agent associated right now; is the trip done?",
            trip
        );
        State::Active(time, trip, None)
    }
}

fn debug_all_routes(ctx: &mut PluginCtx) -> State {
    let sim = &ctx.primary.sim;
    let mut traces: Vec<Trace> = Vec::new();
    for trip in sim.get_stats().canonical_pt_per_trip.keys() {
        if let Some(agent) = sim.trip_to_agent(*trip) {
            if let Some(trace) = sim.trace_route(agent, &ctx.primary.map, f64::MAX * si::M) {
                traces.push(trace);
            }
        }
    }
    State::DebugAllRoutes(sim.time, traces)
}
