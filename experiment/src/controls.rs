use geom::{Angle, Pt2D, Speed};
use widgetry::{EventCtx, Key};

// TODO The timestep accumulation seems fine. What's wrong? Clamping errors repeated?
const HACK: f64 = 5.0;

pub trait Controller {
    fn displacement(&mut self, ctx: &mut EventCtx, speed: Speed) -> (f64, f64);
}

pub struct InstantController;

impl InstantController {
    pub fn new() -> InstantController {
        InstantController
    }
}

impl Controller for InstantController {
    fn displacement(&mut self, ctx: &mut EventCtx, speed: Speed) -> (f64, f64) {
        let mut dx = 0.0;
        let mut dy = 0.0;

        if let Some(dt) = ctx.input.nonblocking_is_update_event() {
            let dist = (dt * HACK * speed).inner_meters();
            if ctx.is_key_down(Key::LeftArrow) {
                dx -= dist;
            }
            if ctx.is_key_down(Key::RightArrow) {
                dx += dist;
            }
            if ctx.is_key_down(Key::UpArrow) {
                dy -= dist;
            }
            if ctx.is_key_down(Key::DownArrow) {
                dy += dist;
            }
        }

        (dx, dy)
    }
}

pub struct RotateController {
    angle: Angle,
}

impl RotateController {
    pub fn new() -> RotateController {
        RotateController { angle: Angle::ZERO }
    }
}

impl Controller for RotateController {
    fn displacement(&mut self, ctx: &mut EventCtx, fwd_speed: Speed) -> (f64, f64) {
        let rot_speed_degrees = 100.0;

        let mut dx = 0.0;
        let mut dy = 0.0;

        if let Some(dt) = ctx.input.nonblocking_is_update_event() {
            if ctx.is_key_down(Key::LeftArrow) {
                self.angle = self
                    .angle
                    .rotate_degs(-rot_speed_degrees * dt.inner_seconds());
            }
            if ctx.is_key_down(Key::RightArrow) {
                self.angle = self
                    .angle
                    .rotate_degs(rot_speed_degrees * dt.inner_seconds());
            }

            if ctx.is_key_down(Key::UpArrow) {
                let dist = dt * HACK * fwd_speed;
                let pt = Pt2D::new(0.0, 0.0).project_away(dist, self.angle);
                dx = pt.x();
                dy = pt.y();
            }
        }

        (dx, dy)
    }
}
