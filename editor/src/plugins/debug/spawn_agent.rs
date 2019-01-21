use crate::objects::{Ctx, ID};
use crate::plugins::{Plugin, PluginCtx};
use dimensioned::si;
use ezgui::{Color, GfxCtx, Key};
use map_model::{BuildingID, LaneID, PathRequest, Pathfinder, Position, Trace, LANE_THICKNESS};
use std::f64;

enum Source {
    Walking(BuildingID),
    Driving(LaneID),
}

enum Goal {
    Building(BuildingID),
}

pub struct SpawnAgent {
    from: Source,
    maybe_goal: Option<(Goal, Option<Trace>)>,
}

impl SpawnAgent {
    pub fn new(ctx: &mut PluginCtx) -> Option<SpawnAgent> {
        match ctx.primary.current_selection {
            Some(ID::Building(id)) => {
                if ctx
                    .input
                    .contextual_action(Key::F3, "spawn an agent starting here")
                {
                    return Some(SpawnAgent {
                        from: Source::Walking(id),
                        maybe_goal: None,
                    });
                }
            }
            Some(ID::Lane(id)) => {
                if ctx.primary.map.get_l(id).is_driving()
                    && ctx
                        .input
                        .contextual_action(Key::F3, "spawn an agent starting here")
                {
                    return Some(SpawnAgent {
                        from: Source::Driving(id),
                        maybe_goal: None,
                    });
                }
            }
            _ => {}
        }
        None
    }
}

impl Plugin for SpawnAgent {
    fn blocking_event(&mut self, ctx: &mut PluginCtx) -> bool {
        ctx.input.set_mode("Agent Spawner", &ctx.canvas);
        if ctx.input.modal_action("quit") {
            return false;
        }
        let map = &ctx.primary.map;

        if let Some(ID::Building(to)) = ctx.primary.current_selection {
            let recalculate = match self.maybe_goal {
                Some((Goal::Building(b), _)) => to != b,
                None => true,
            };
            if recalculate {
                self.maybe_goal = Some((Goal::Building(to), None));

                let (start, end) = match self.from {
                    Source::Walking(from) => (
                        map.get_b(from).front_path.sidewalk,
                        map.get_b(to).front_path.sidewalk,
                    ),
                    Source::Driving(from) => {
                        let end = map.find_driving_lane_near_building(to);
                        (
                            Position::new(from, 0.0 * si::M),
                            Position::new(end, map.get_l(end).length()),
                        )
                    }
                };
                if let Some(path) = Pathfinder::shortest_distance(
                    map,
                    PathRequest {
                        start,
                        end,
                        can_use_bike_lanes: false,
                        can_use_bus_lanes: false,
                    },
                ) {
                    self.maybe_goal = Some((
                        Goal::Building(to),
                        path.trace(map, start.dist_along(), f64::MAX * si::M),
                    ));
                }
            }
        } else {
            self.maybe_goal = None;
        }

        match self.maybe_goal {
            Some((Goal::Building(to), _)) => {
                if ctx.input.contextual_action(Key::F3, "end the agent here") {
                    match self.from {
                        Source::Walking(from) => {
                            info!(
                                "Spawning {}",
                                ctx.primary.sim.seed_trip_just_walking(from, to, map)
                            );
                        }
                        Source::Driving(from) => {
                            info!(
                                "Spawning {}",
                                ctx.primary.sim.seed_trip_with_car_appearing(from, to, map)
                            );
                        }
                    };
                    return false;
                }
            }
            _ => {}
        };

        true
    }

    fn draw(&self, g: &mut GfxCtx, ctx: &Ctx) {
        if let Some((_, Some(ref trace))) = self.maybe_goal {
            g.draw_polygon(ctx.cs.get("route"), &trace.make_polygons(LANE_THICKNESS));
        }
    }

    fn color_for(&self, obj: ID, ctx: &Ctx) -> Option<Color> {
        match (&self.from, obj) {
            (Source::Walking(ref b1), ID::Building(b2)) if *b1 == b2 => {
                Some(ctx.cs.get("selected"))
            }
            (Source::Driving(ref l1), ID::Lane(l2)) if *l1 == l2 => Some(ctx.cs.get("selected")),
            _ => None,
        }
    }
}
