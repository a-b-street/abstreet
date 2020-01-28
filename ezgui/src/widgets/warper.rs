use crate::{EventCtx, EventLoopMode};
use geom::{Line, Pt2D};
use std::time::Instant;

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

    // None means done
    pub fn event(&self, ctx: &mut EventCtx) -> Option<EventLoopMode> {
        // Actually nothing for us to do
        if self.line.is_none() && self.cam_zoom.0 == self.cam_zoom.1 {
            return None;
        }

        // Weird to do stuff for any event?
        if ctx.input.nonblocking_is_update_event().is_none() {
            return Some(EventLoopMode::Animation);
        }
        ctx.input.use_update_event();

        const MAX_ANIMATION_TIME_S: f64 = 0.5;
        const ANIMATION_SPEED: f64 = 200.0;
        let total_time = if let Some(ref line) = self.line {
            (line.length().inner_meters() / ANIMATION_SPEED).min(MAX_ANIMATION_TIME_S)
        } else {
            MAX_ANIMATION_TIME_S
        };
        let percent = abstutil::elapsed_seconds(self.started) / total_time;

        let orig_center = ctx.canvas.center_to_map_pt();
        if percent >= 1.0 || ctx.input.nonblocking_is_keypress_event() {
            ctx.canvas.cam_zoom = self.cam_zoom.1;
            if let Some(ref line) = self.line {
                ctx.canvas.center_on_map_pt(line.pt2());
            } else {
                ctx.canvas.center_on_map_pt(orig_center);
            }
            None
        } else {
            ctx.canvas.cam_zoom = self.cam_zoom.0 + percent * (self.cam_zoom.1 - self.cam_zoom.0);
            if let Some(ref line) = self.line {
                ctx.canvas
                    .center_on_map_pt(line.dist_along(line.length() * percent));
            } else {
                ctx.canvas.center_on_map_pt(orig_center);
            }
            Some(EventLoopMode::Animation)
        }
    }
}
