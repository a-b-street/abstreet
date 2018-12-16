use crate::objects::{Ctx, DEBUG};
use crate::plugins::{Plugin, PluginCtx};
use ezgui::{Color, GfxCtx, Key};
use geom::{Bounds, Polygon, Pt2D};
use sim::{Sim, Tick};

pub struct ShowActivityState {
    state: State,
    key: Key,
}

enum State {
    Inactive,
    Active(Tick, Heatmap),
}

impl ShowActivityState {
    pub fn new(key: Key) -> ShowActivityState {
        ShowActivityState {
            state: State::Inactive,
            key,
        }
    }
}

impl Plugin for ShowActivityState {
    fn ambient_event(&mut self, ctx: &mut PluginCtx) {
        match self.state {
            State::Inactive => {
                if ctx.input.unimportant_key_pressed(
                    self.key,
                    DEBUG,
                    "show lanes with active traffic",
                ) {
                    self.state = State::Active(
                        ctx.primary.sim.time,
                        active_agent_heatmap(ctx.canvas.get_screen_bounds(), &ctx.primary.sim),
                    );
                }
            }
            State::Active(time, ref old_heatmap) => {
                if ctx
                    .input
                    .key_pressed(self.key, "stop showing lanes with active traffic")
                {
                    self.state = State::Inactive;
                    return;
                }
                let bounds = ctx.canvas.get_screen_bounds();
                if time != ctx.primary.sim.time || bounds != old_heatmap.bounds {
                    self.state = State::Active(
                        ctx.primary.sim.time,
                        active_agent_heatmap(bounds, &ctx.primary.sim),
                    );
                }
            }
        }
    }

    fn draw(&self, g: &mut GfxCtx, _ctx: &Ctx) {
        if let State::Active(_, ref heatmap) = self.state {
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
                let color = Color::rgba(255, 0, 0, percent * 0.8);
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
