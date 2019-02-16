use crate::objects::DrawCtx;
use crate::plugins::{AmbientPlugin, PluginCtx};
use crate::render::{DrawRoad, MIN_ZOOM_FOR_DETAIL};
use ezgui::{Color, GfxCtx};
use geom::{Bounds, Polygon, Pt2D};
use map_model::{RoadID, Traversable};
use sim::Tick;
use std::collections::HashMap;

pub enum ShowActivityState {
    Inactive,
    Active(Tick, Heatmap, RoadHeatmap),
}

impl ShowActivityState {
    pub fn new() -> ShowActivityState {
        ShowActivityState::Inactive
    }
}

impl AmbientPlugin for ShowActivityState {
    fn ambient_event(&mut self, ctx: &mut PluginCtx) {
        match self {
            ShowActivityState::Inactive => {
                if ctx.input.action_chosen("show lanes with active traffic") {
                    *self = ShowActivityState::Active(
                        ctx.primary.sim.time,
                        active_agent_heatmap(ctx),
                        RoadHeatmap::new(ctx),
                    );
                }
            }
            ShowActivityState::Active(time, ref old_heatmap, _) => {
                ctx.input.set_mode("Active Traffic Visualizer", &ctx.canvas);
                if ctx.input.modal_action("quit") {
                    *self = ShowActivityState::Inactive;
                    return;
                }
                let bounds = ctx.canvas.get_screen_bounds();
                if *time != ctx.primary.sim.time || bounds != old_heatmap.bounds {
                    *self = ShowActivityState::Active(
                        ctx.primary.sim.time,
                        active_agent_heatmap(ctx),
                        RoadHeatmap::new(ctx),
                    );
                }
            }
        }
    }

    fn draw(&self, g: &mut GfxCtx, ctx: &DrawCtx) {
        if let ShowActivityState::Active(_, ref heatmap, ref road_heatmap) = self {
            if g.canvas.cam_zoom < MIN_ZOOM_FOR_DETAIL {
                road_heatmap.draw(g, ctx);
            } else {
                heatmap.draw(g);
            }
        }
    }
}

// A nice 10x10
const NUM_TILES: usize = 10;

pub struct Heatmap {
    bounds: Bounds,

    counts: [[usize; NUM_TILES]; NUM_TILES],
    max: usize,
}

impl Heatmap {
    fn new(bounds: Bounds) -> Heatmap {
        Heatmap {
            bounds,
            counts: [[0; NUM_TILES]; NUM_TILES],
            max: 0,
        }
    }

    fn add(&mut self, pt: Pt2D) {
        // TODO Could also query sim with this filter
        if !self.bounds.contains(pt) {
            return;
        }

        let x = ((pt.x() - self.bounds.min_x) / (self.bounds.max_x - self.bounds.min_x)
            * (NUM_TILES as f64))
            .floor() as usize;
        let y = ((pt.y() - self.bounds.min_y) / (self.bounds.max_y - self.bounds.min_y)
            * (NUM_TILES as f64))
            .floor() as usize;
        self.counts[x][y] += 1;
        self.max = self.max.max(self.counts[x][y]);
    }

    fn draw(&self, g: &mut GfxCtx) {
        let tile_width = (self.bounds.max_x - self.bounds.min_x) / (NUM_TILES as f64);
        let tile_height = (self.bounds.max_y - self.bounds.min_y) / (NUM_TILES as f64);

        for x in 0..NUM_TILES {
            for y in 0..NUM_TILES {
                if self.counts[x][y] == 0 {
                    continue;
                }

                let percent = (self.counts[x][y] as f32) / (self.max as f32);
                // TODO Map percent to hot/cold colors. For now, don't ever become totally opaque.
                let color = Color::RED.alpha(percent * 0.8);
                g.draw_polygon(
                    color,
                    &Polygon::rectangle_topleft(
                        Pt2D::new(
                            self.bounds.min_x + (x as f64) * tile_width,
                            self.bounds.min_y + (y as f64) * tile_height,
                        ),
                        tile_width,
                        tile_height,
                    ),
                );
            }
        }
    }
}

fn active_agent_heatmap(ctx: &mut PluginCtx) -> Heatmap {
    let mut h = Heatmap::new(ctx.canvas.get_screen_bounds());
    let stats = ctx.primary.sim.get_stats(&ctx.primary.map);
    for pt in stats.canonical_pt_per_trip.values() {
        h.add(*pt);
    }
    h
}

pub struct RoadHeatmap {
    // TODO Use the Counter type? Roll my own simple one?
    count_per_road: HashMap<RoadID, usize>,
    max_count: usize,
}

impl RoadHeatmap {
    fn new(ctx: &mut PluginCtx) -> RoadHeatmap {
        let mut h = RoadHeatmap {
            count_per_road: HashMap::new(),
            max_count: 0,
        };
        let map = &ctx.primary.map;
        for a in ctx.primary.sim.active_agents() {
            let r = match ctx.primary.sim.location_for_agent(a, map) {
                Traversable::Lane(l) => map.get_l(l).parent,
                // Count the destination
                Traversable::Turn(t) => map.get_l(t.dst).parent,
            };
            h.count_per_road.entry(r).or_insert(0);
            let count = h.count_per_road[&r] + 1;
            h.count_per_road.insert(r, count);
            h.max_count = h.max_count.max(count);
        }
        h
    }

    fn draw(&self, g: &mut GfxCtx, ctx: &DrawCtx) {
        for (r, count) in &self.count_per_road {
            let percent = (*count as f32) / (self.max_count as f32);
            // TODO Map percent to hot/cold colors. For now, just bucket into 3 categories.
            let color = if percent <= 0.3 {
                Color::rgb(255, 255, 0)
            } else if percent <= 0.6 {
                Color::rgb(255, 128, 0)
            } else {
                Color::RED
            };
            // TODO Inefficient!
            g.draw_polygon(color, &DrawRoad::get_thick(ctx.map.get_r(*r)));
        }
    }
}
