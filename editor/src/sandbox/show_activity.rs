use crate::render::MIN_ZOOM_FOR_DETAIL;
use crate::ui::UI;
use ezgui::{Color, EventCtx, GfxCtx, ModalMenu};
use geom::{Bounds, Distance, Duration, Polygon, Pt2D};
use map_model::{RoadID, Traversable};
use std::collections::HashMap;

pub enum ShowActivity {
    Inactive,
    Unzoomed(Duration, RoadHeatmap),
    Zoomed(Duration, Heatmap),
}

impl ShowActivity {
    pub fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI, menu: &mut ModalMenu) {
        let zoomed = ctx.canvas.cam_zoom >= MIN_ZOOM_FOR_DETAIL;

        // If we survive past this, recompute current state.
        match self {
            ShowActivity::Inactive => {
                if !menu.action("show/hide active traffic") {
                    return;
                }
            }
            ShowActivity::Zoomed(time, ref heatmap) => {
                if menu.action("show/hide active traffic") {
                    *self = ShowActivity::Inactive;
                    return;
                }
                if *time == ui.primary.sim.time()
                    && ctx.canvas.get_screen_bounds() == heatmap.bounds
                    && zoomed
                {
                    return;
                }
            }
            ShowActivity::Unzoomed(time, _) => {
                if menu.action("show/hide active traffic") {
                    *self = ShowActivity::Inactive;
                    return;
                }
                if *time == ui.primary.sim.time() && !zoomed {
                    return;
                }
            }
        };

        if zoomed {
            *self = ShowActivity::Zoomed(ui.primary.sim.time(), active_agent_heatmap(ctx, ui));
        } else {
            *self = ShowActivity::Unzoomed(ui.primary.sim.time(), RoadHeatmap::new(ui));
        }
    }

    pub fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        match self {
            ShowActivity::Zoomed(_, ref heatmap) => {
                heatmap.draw(g);
            }
            ShowActivity::Unzoomed(_, ref road_heatmap) => {
                road_heatmap.draw(g, ui);
            }
            ShowActivity::Inactive => {}
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
                        Distance::meters(tile_width),
                        Distance::meters(tile_height),
                    ),
                );
            }
        }
    }
}

fn active_agent_heatmap(ctx: &EventCtx, ui: &mut UI) -> Heatmap {
    let mut h = Heatmap::new(ctx.canvas.get_screen_bounds());
    let stats = ui.primary.sim.get_stats(&ui.primary.map);
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
    fn new(ui: &UI) -> RoadHeatmap {
        let mut h = RoadHeatmap {
            count_per_road: HashMap::new(),
            max_count: 0,
        };
        let map = &ui.primary.map;
        for a in ui.primary.sim.active_agents() {
            let r = match ui.primary.sim.location_for_agent(a, map) {
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

    fn draw(&self, g: &mut GfxCtx, ui: &UI) {
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
            g.draw_polygon(
                color,
                &ui.primary.map.get_r(*r).get_thick_polygon().unwrap(),
            );
        }
    }
}
