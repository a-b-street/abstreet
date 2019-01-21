use crate::objects::{Ctx, ID};
use crate::plugins::{Plugin, PluginCtx};
use dimensioned::si;
use ezgui::{Color, GfxCtx, Key};
use map_model::{BuildingID, LaneID, PathRequest, Pathfinder, Position, Trace, LANE_THICKNESS};
use std::f64;

// TODO Don't like the duplicated logic here.
pub enum SpawnAgent {
    Walking(BuildingID, Option<(BuildingID, Option<Trace>)>),
    Driving(LaneID, Option<(BuildingID, Option<Trace>)>),
}

impl SpawnAgent {
    pub fn new(ctx: &mut PluginCtx) -> Option<SpawnAgent> {
        match ctx.primary.current_selection {
            Some(ID::Building(id)) => {
                if ctx
                    .input
                    .contextual_action(Key::F3, "spawn an agent starting here")
                {
                    return Some(SpawnAgent::Walking(id, None));
                }
            }
            Some(ID::Lane(id)) => {
                if ctx.primary.map.get_l(id).is_driving()
                    && ctx
                        .input
                        .contextual_action(Key::F3, "spawn an agent starting here")
                {
                    return Some(SpawnAgent::Driving(id, None));
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

        match self {
            SpawnAgent::Walking(ref raw_from, ref maybe_to) => {
                let from = raw_from.clone();
                if let Some(ID::Building(id)) = ctx.primary.current_selection {
                    let map = &ctx.primary.map;
                    if maybe_to.as_ref().map(|(b, _)| *b != id).unwrap_or(true) {
                        *self = SpawnAgent::Walking(from, Some((id, None)));

                        let start = map.get_b(from).front_path.sidewalk;
                        if let Some(path) = Pathfinder::shortest_distance(
                            map,
                            PathRequest {
                                start,
                                end: map.get_b(id).front_path.sidewalk,
                                can_use_bike_lanes: false,
                                can_use_bus_lanes: false,
                            },
                        ) {
                            *self = SpawnAgent::Walking(
                                from,
                                Some((id, path.trace(map, start.dist_along(), f64::MAX * si::M))),
                            );
                        }
                    }

                    if ctx.input.contextual_action(Key::F3, "end the agent here") {
                        info!(
                            "Spawning {}",
                            ctx.primary.sim.seed_trip_just_walking(from, id, map)
                        );
                        return false;
                    }
                } else {
                    *self = SpawnAgent::Walking(from, None);
                }
            }
            SpawnAgent::Driving(ref raw_from, ref maybe_to) => {
                let from = raw_from.clone();
                if let Some(ID::Building(id)) = ctx.primary.current_selection {
                    let map = &ctx.primary.map;
                    if maybe_to.as_ref().map(|(b, _)| *b != id).unwrap_or(true) {
                        *self = SpawnAgent::Driving(from, Some((id, None)));

                        let end = map.find_driving_lane_near_building(id);
                        if let Some(path) = Pathfinder::shortest_distance(
                            map,
                            PathRequest {
                                start: Position::new(from, 0.0 * si::M),
                                end: Position::new(end, map.get_l(end).length()),
                                can_use_bike_lanes: false,
                                can_use_bus_lanes: false,
                            },
                        ) {
                            *self = SpawnAgent::Driving(
                                from,
                                Some((id, path.trace(map, 0.0 * si::M, f64::MAX * si::M))),
                            );
                        }
                    }

                    if ctx.input.contextual_action(Key::F3, "end the agent here") {
                        info!(
                            "Spawning {}",
                            ctx.primary.sim.seed_trip_with_car_appearing(from, id, map)
                        );
                        return false;
                    }
                } else {
                    *self = SpawnAgent::Driving(from, None);
                }
            }
        };

        true
    }

    fn draw(&self, g: &mut GfxCtx, ctx: &Ctx) {
        match self {
            SpawnAgent::Walking(_, Some((_, Some(ref trace))))
            | SpawnAgent::Driving(_, Some((_, Some(ref trace)))) => {
                g.draw_polygon(ctx.cs.get("route"), &trace.make_polygons(LANE_THICKNESS));
            }
            _ => {}
        }
    }

    fn color_for(&self, obj: ID, ctx: &Ctx) -> Option<Color> {
        match (self, obj) {
            (SpawnAgent::Walking(b1, _), ID::Building(b2)) if *b1 == b2 => {
                Some(ctx.cs.get("selected"))
            }
            (SpawnAgent::Driving(l1, _), ID::Lane(l2)) if *l1 == l2 => Some(ctx.cs.get("selected")),
            _ => None,
        }
    }
}
