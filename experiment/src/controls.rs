use geom::{Angle, Pt2D, Speed};
use widgetry::{EventCtx, Key};

pub trait Controller {
    fn displacement(&mut self, ctx: &mut EventCtx) -> (f64, f64);
}

pub struct InstantController {
    speed: Speed,
}

impl InstantController {
    pub fn new(speed: Speed) -> InstantController {
        InstantController {
            // TODO Hack
            speed: 5.0 * speed,
        }
    }
}

impl Controller for InstantController {
    fn displacement(&mut self, ctx: &mut EventCtx) -> (f64, f64) {
        let mut dx = 0.0;
        let mut dy = 0.0;

        if let Some(dt) = ctx.input.nonblocking_is_update_event() {
            ctx.input.use_update_event();

            let dist = (dt * self.speed).inner_meters();
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
    rot_speed_degrees: f64,
    fwd_speed: Speed,
}

impl RotateController {
    pub fn new(fwd_speed: Speed) -> RotateController {
        RotateController {
            angle: Angle::ZERO,
            rot_speed_degrees: 100.0,
            // TODO Hack
            fwd_speed: 5.0 * fwd_speed,
        }
    }
}

impl Controller for RotateController {
    fn displacement(&mut self, ctx: &mut EventCtx) -> (f64, f64) {
        let mut dx = 0.0;
        let mut dy = 0.0;

        if let Some(dt) = ctx.input.nonblocking_is_update_event() {
            ctx.input.use_update_event();

            if ctx.is_key_down(Key::LeftArrow) {
                self.angle = self
                    .angle
                    .rotate_degs(-self.rot_speed_degrees * dt.inner_seconds());
            }
            if ctx.is_key_down(Key::RightArrow) {
                self.angle = self
                    .angle
                    .rotate_degs(self.rot_speed_degrees * dt.inner_seconds());
            }

            if ctx.is_key_down(Key::UpArrow) {
                let dist = dt * self.fwd_speed;
                let pt = Pt2D::new(0.0, 0.0).project_away(dist, self.angle);
                dx = pt.x();
                dy = pt.y();
            }
        }

        (dx, dy)
    }
}
