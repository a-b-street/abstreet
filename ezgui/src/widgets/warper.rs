use crate::{EventCtx, EventLoopMode};
use geom::{Line, Pt2D};
use std::time::Instant;

const ANIMATION_TIME_S: f64 = 0.5;
// TODO Should factor in zoom too
const MIN_ANIMATION_SPEED: f64 = 200.0;

pub struct Warper {
    started: Instant,
    line: Option<Line>,
    cam_zoom: (f64, f64),
}

impl Warper {
    pub fn new(ctx: &EventCtx, pt: Pt2D, target_cam_zoom: Option<f64>) -> Warper {
        let z = ctx.canvas.cam_zoom;
        Warper {
            started: Instant::now(),
            line: Line::maybe_new(ctx.canvas.center_to_map_pt(), pt),
            cam_zoom: (z, target_cam_zoom.unwrap_or(z)),
        }
    }

    pub fn event(&self, ctx: &mut EventCtx) -> Option<EventLoopMode> {
        let line = self.line.as_ref()?;

        // Weird to do stuff for any event?
        if ctx.input.nonblocking_is_update_event() {
            ctx.input.use_update_event();
        }

        let speed = line.length().inner_meters() / ANIMATION_TIME_S;
        let total_time = if speed >= MIN_ANIMATION_SPEED {
            ANIMATION_TIME_S
        } else {
            line.length().inner_meters() / MIN_ANIMATION_SPEED
        };
        let percent = abstutil::elapsed_seconds(self.started) / total_time;

        if percent >= 1.0 || ctx.input.nonblocking_is_keypress_event() {
            ctx.canvas.cam_zoom = self.cam_zoom.1;
            ctx.canvas.center_on_map_pt(line.pt2());
            None
        } else {
            ctx.canvas.cam_zoom = self.cam_zoom.0 + percent * (self.cam_zoom.1 - self.cam_zoom.0);
            ctx.canvas
                .center_on_map_pt(line.dist_along(line.length() * percent));
            Some(EventLoopMode::Animation)
        }
    }
}
