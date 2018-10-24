use dimensioned::si;
use ezgui::{Color, GfxCtx};
use geom::Line;
use map_model::{Trace, LANE_THICKNESS};
use objects::Ctx;
use piston::input::Key;
use plugins::{Plugin, PluginCtx};
use sim::TripID;
use std::f64;

pub enum DiffWorldsState {
    Inactive,
    Active {
        trip: TripID,
        // These are all optional because mode-changes might cause temporary interruptions.
        // Just point from primary world agent to secondary world agent.
        line: Option<Line>,
        primary_route: Option<Trace>,
        secondary_route: Option<Trace>,
    },
}

impl DiffWorldsState {
    pub fn new() -> DiffWorldsState {
        DiffWorldsState::Inactive
    }
}

impl Plugin for DiffWorldsState {
    fn event(&mut self, ctx: PluginCtx) -> bool {
        let mut maybe_trip: Option<TripID> = None;
        match self {
            DiffWorldsState::Inactive => {
                if ctx.secondary.is_some() {
                    if let Some(id) = ctx.primary.current_selection.and_then(|id| id.agent_id()) {
                        if let Some(trip) = ctx.primary.sim.agent_to_trip(id) {
                            if ctx
                                .input
                                .key_pressed(Key::B, &format!("Show {}'s parallel world", trip))
                            {
                                maybe_trip = Some(trip);
                            }
                        }
                    }
                }
            }
            DiffWorldsState::Active { trip, .. } => {
                if ctx.input.key_pressed(
                    Key::Return,
                    &format!("Stop showing {}'s parallel world", trip),
                ) {
                    maybe_trip = None;
                } else {
                    maybe_trip = Some(*trip);
                }
            }
        }

        if let Some(trip) = maybe_trip {
            let primary_sim = &ctx.primary.sim;
            let primary_map = &ctx.primary.map;
            let (secondary_sim, secondary_map) = ctx
                .secondary
                .as_ref()
                .map(|(s, _)| (&s.sim, &s.map))
                .unwrap();

            let pt1 = primary_sim.get_canonical_point_for_trip(trip, primary_map);
            let pt2 = secondary_sim.get_canonical_point_for_trip(trip, secondary_map);
            let line = if pt1.is_some() && pt2.is_some() {
                Some(Line::new(pt1.unwrap(), pt2.unwrap()))
            } else {
                None
            };
            let primary_route = primary_sim
                .trip_to_agent(trip)
                .and_then(|agent| primary_sim.trace_route(agent, primary_map, f64::MAX * si::M));
            let secondary_route = secondary_sim.trip_to_agent(trip).and_then(|agent| {
                secondary_sim.trace_route(agent, secondary_map, f64::MAX * si::M)
            });

            if line.is_none() || primary_route.is_none() || secondary_route.is_none() {
                warn!("{} isn't present in both sims", trip);
            }
            *self = DiffWorldsState::Active {
                trip,
                line,
                primary_route,
                secondary_route,
            };
        } else {
            *self = DiffWorldsState::Inactive;
        }

        match self {
            DiffWorldsState::Inactive => false,
            _ => true,
        }
    }

    fn draw(&self, g: &mut GfxCtx, ctx: Ctx) {
        if let DiffWorldsState::Active {
            line,
            primary_route,
            secondary_route,
            ..
        } = self
        {
            if let Some(l) = line {
                g.draw_line(
                    ctx.cs.get("diff agents line", Color::YELLOW),
                    LANE_THICKNESS,
                    l,
                );
            }
            if let Some(t) = primary_route {
                g.draw_polygon(
                    ctx.cs
                        .get("primary agent route", Color::rgba(255, 0, 0, 0.5)),
                    &t.get_polyline().make_polygons_blindly(LANE_THICKNESS),
                );
            }
            if let Some(t) = secondary_route {
                g.draw_polygon(
                    ctx.cs
                        .get("secondary agent route", Color::rgba(0, 0, 255, 0.5)),
                    &t.get_polyline().make_polygons_blindly(LANE_THICKNESS),
                );
            }
        }
    }
}
