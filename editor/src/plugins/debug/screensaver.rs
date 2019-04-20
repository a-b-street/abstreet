use crate::plugins::{BlockingPlugin, PluginCtx};
use abstutil::elapsed_seconds;
use ezgui::EventLoopMode;
use geom::{Duration, Line, Pt2D, Speed};
use rand::Rng;
use rand_xorshift::XorShiftRng;
use std::time::Instant;

const SPEED: Speed = Speed::const_meters_per_second(50.0);

pub struct Screensaver {
    line: Line,
    started: Instant,
    rng: XorShiftRng,
}

impl Screensaver {
    pub fn new(ctx: &mut PluginCtx) -> Option<Screensaver> {
        if ctx.input.action_chosen("screensaver mode") {
            let mut rng = ctx.primary.current_flags.sim_flags.make_rng();
            ctx.hints.mode = EventLoopMode::Animation;

            let at = ctx.canvas.center_to_map_pt();
            ctx.canvas.cam_zoom = 10.0;
            ctx.canvas.center_on_map_pt(at);

            return Some(Screensaver {
                line: Screensaver::bounce(ctx, &mut rng),
                started: Instant::now(),
                rng,
            });
        }
        None
    }

    fn bounce(ctx: &mut PluginCtx, rng: &mut XorShiftRng) -> Line {
        let bounds = ctx.primary.map.get_bounds();
        let at = ctx.canvas.center_to_map_pt();
        // TODO Ideally bounce off the edge of the map
        let goto = Pt2D::new(
            rng.gen_range(0.0, bounds.max_x),
            rng.gen_range(0.0, bounds.max_y),
        );
        Line::new(at, goto)
    }
}

impl BlockingPlugin for Screensaver {
    fn blocking_event(&mut self, ctx: &mut PluginCtx) -> bool {
        ctx.input.set_mode("Screensaver", &ctx.canvas);
        if ctx.input.modal_action("quit") {
            return false;
        }

        if ctx.input.nonblocking_is_update_event() {
            ctx.input.use_update_event();
            let dist_along = Duration::seconds(elapsed_seconds(self.started)) * SPEED;
            if dist_along < self.line.length() {
                ctx.canvas
                    .center_on_map_pt(self.line.dist_along(dist_along));
            } else {
                self.line = Screensaver::bounce(ctx, &mut self.rng);
                self.started = Instant::now();
            }
            ctx.hints.mode = EventLoopMode::Animation;
        }
        true
    }
}
