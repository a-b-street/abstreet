use crate::objects::{Ctx, ID};
use crate::plugins::{Plugin, PluginCtx};
use dimensioned::si;
use ezgui::{Color, GfxCtx, Key};
use map_model::{BuildingID, PathRequest, Pathfinder, Trace, LANE_THICKNESS};
use std::f64;

pub struct SpawnAgent {
    from_bldg: BuildingID,

    maybe_goal: Option<(BuildingID, Option<Trace>)>,
}

impl SpawnAgent {
    pub fn new(ctx: &mut PluginCtx) -> Option<SpawnAgent> {
        if let Some(ID::Building(id)) = ctx.primary.current_selection {
            if ctx
                .input
                .contextual_action(Key::F3, "spawn an agent starting here")
            {
                return Some(SpawnAgent {
                    from_bldg: id,
                    maybe_goal: None,
                });
            }
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

        // TODO disabling mouseover at low zoom is actually annoying now
        if let Some(ID::Building(id)) = ctx.primary.current_selection {
            if self
                .maybe_goal
                .as_ref()
                .map(|(b, _)| *b != id)
                .unwrap_or(true)
            {
                self.maybe_goal = Some((id, None));

                let map = &ctx.primary.map;
                let start = map.get_b(self.from_bldg).front_path.sidewalk;
                if let Some(path) = Pathfinder::shortest_distance(
                    map,
                    PathRequest {
                        start,
                        end: map.get_b(id).front_path.sidewalk,
                        can_use_bike_lanes: false,
                        can_use_bus_lanes: false,
                    },
                ) {
                    self.maybe_goal =
                        Some((id, path.trace(map, start.dist_along(), f64::MAX * si::M)));
                }
            }
        } else {
            self.maybe_goal = None;
        }

        if self.maybe_goal.is_some() && ctx.input.contextual_action(Key::F3, "end the agent here") {
            // TODO spawn em
            return false;
        }

        true
    }

    fn draw(&self, g: &mut GfxCtx, ctx: &Ctx) {
        if let Some((_, Some(ref trace))) = self.maybe_goal {
            g.draw_polygon(ctx.cs.get("route"), &trace.make_polygons(LANE_THICKNESS));
        }
    }

    fn color_for(&self, obj: ID, ctx: &Ctx) -> Option<Color> {
        if ID::Building(self.from_bldg) == obj {
            Some(ctx.cs.get("selected"))
        } else {
            None
        }
    }
}
