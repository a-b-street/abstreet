use ezgui::{Color, GfxCtx};
use geom::{Bounds, Pt2D};
use map_model::Map;
use objects::{Ctx, DEBUG};
use piston::input::Key;
use plugins::{Plugin, PluginCtx};
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
    fn event(&mut self, ctx: PluginCtx) -> bool {
        let mut new_state: Option<ShowActivityState> = None;
        match self {
            ShowActivityState::Inactive => {
                if ctx.input.unimportant_key_pressed(
                    Key::A,
                    DEBUG,
                    "show lanes with active traffic",
                ) {
                    new_state = Some(ShowActivityState::Active(
                        ctx.primary.sim.time,
                        active_agent_heatmap(
                            ctx.canvas.get_screen_bounds(),
                            &ctx.primary.sim,
                            &ctx.primary.map,
                        ),
                    ));
                }
            }
            ShowActivityState::Active(time, ref old_heatmap) => {
                if ctx
                    .input
                    .key_pressed(Key::Return, "stop showing lanes with active traffic")
                {
                    new_state = Some(ShowActivityState::Inactive);
                }
                let bounds = ctx.canvas.get_screen_bounds();
                if *time != ctx.primary.sim.time || bounds != old_heatmap.bounds {
                    new_state = Some(ShowActivityState::Active(
                        ctx.primary.sim.time,
                        active_agent_heatmap(bounds, &ctx.primary.sim, &ctx.primary.map),
                    ));
                }
            }
        }
        if let Some(s) = new_state {
            *self = s;
        }
        match self {
            ShowActivityState::Inactive => false,
            _ => true,
        }
    }

    fn draw(&self, g: &mut GfxCtx, _ctx: Ctx) {
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
                // TODO Map percent to hot/cold colors
                let color = Color::rgba(255, 0, 0, percent);
                g.draw_rectangle(
                    color,
                    [
                        (x as f64) * tile_width,
                        (y as f64) * tile_height,
                        tile_width,
                        tile_height,
                    ],
                );
            }
        }
    }
}

fn active_agent_heatmap(bounds: Bounds, sim: &Sim, map: &Map) -> Heatmap {
    let mut h = Heatmap::new(bounds);
    for trip in sim.get_active_trips().into_iter() {
        if let Some(pt) = sim.get_canonical_point_for_trip(trip, map) {
            h.add(pt);
        }
    }
    h
}
