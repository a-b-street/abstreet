use crate::{EventCtx, EventLoopMode};
use geom::{Line, Pt2D};
use std::time::Instant;

const ANIMATION_TIME_S: f64 = 0.5;
// TODO Should factor in zoom too
const MIN_ANIMATION_SPEED: f64 = 200.0;

pub struct Warper {
    started: Instant,
    line: Option<Line>,
}

impl Warper {
    pub fn new(ctx: &EventCtx, pt: Pt2D) -> Warper {
        Warper {
            started: Instant::now(),
            line: Line::maybe_new(ctx.canvas.center_to_map_pt(), pt),
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
            ctx.canvas.center_on_map_pt(line.pt2());
            None
        } else {
            ctx.canvas
                .center_on_map_pt(line.dist_along(line.length() * percent));
            Some(EventLoopMode::Animation)
        }
    }
}
