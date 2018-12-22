use crate::objects::Ctx;
use crate::plugins::{Plugin, PluginCtx};
use ezgui::{Color, GfxCtx};
use geom::{Bounds, Polygon, Pt2D};
use sim::{Sim, Tick};

pub enum ShowActivityState {
    Inactive,
    Active(Tick, Heatmap),
}

impl ShowActivityState {
    pub fn new() -> ShowActivityState {
        ShowActivityState::Inactive
    }
}

impl Plugin for ShowActivityState {
    fn ambient_event(&mut self, ctx: &mut PluginCtx) {
        match self {
            ShowActivityState::Inactive => {
                if ctx.input.action_chosen("show lanes with active traffic") {
                    *self = ShowActivityState::Active(
                        ctx.primary.sim.time,
                        active_agent_heatmap(ctx.canvas.get_screen_bounds(), &ctx.primary.sim),
                    );
                }
            }
            ShowActivityState::Active(time, ref old_heatmap) => {
                ctx.input.set_mode("Active Traffic Visualizer", &ctx.canvas);
                if ctx.input.modal_action("quit") {
                    *self = ShowActivityState::Inactive;
                    return;
                }
                let bounds = ctx.canvas.get_screen_bounds();
                if *time != ctx.primary.sim.time || bounds != old_heatmap.bounds {
                    *self = ShowActivityState::Active(
                        ctx.primary.sim.time,
                        active_agent_heatmap(bounds, &ctx.primary.sim),
                    );
                }
            }
        }
    }

    fn draw(&self, g: &mut GfxCtx, _ctx: &Ctx) {
        if let ShowActivityState::Active(_, ref heatmap) = self {
            heatmap.draw(g);
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

fn active_agent_heatmap(bounds: Bounds, sim: &Sim) -> Heatmap {
    let mut h = Heatmap::new(bounds);
    let stats = sim.get_stats();
    for pt in stats.canonical_pt_per_trip.values() {
        h.add(*pt);
    }
    h
}
