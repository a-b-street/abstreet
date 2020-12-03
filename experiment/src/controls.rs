use geom::{Angle, Speed};
use widgetry::{EventCtx, Key};

// TODO The timestep accumulation seems fine. What's wrong? Clamping errors repeated?
const HACK: f64 = 5.0;

pub struct InstantController {
    /// Which of the 8 directions are we facing, based on the last set of keys pressed down?
    pub facing: Angle,
}

impl InstantController {
    pub fn new() -> InstantController {
        InstantController {
            facing: Angle::ZERO,
        }
    }

    pub fn displacement(&mut self, ctx: &mut EventCtx, speed: Speed) -> (f64, f64) {
        let mut dx = 0.0;
        let mut dy = 0.0;

        if let Some(dt) = ctx.input.nonblocking_is_update_event() {
            // TODO Diagonal movement is faster. Normalize some vectors!
            let dist = (dt * HACK * speed).inner_meters();
            let mut x = 0;
            let mut y = 0;
            // The x direction is inverted; somehow the usual Y inversion did this? Oh well,
            // it's isolated here nicely.
            if ctx.is_key_down(Key::LeftArrow) {
                dx -= dist;
                x = 1;
            }
            if ctx.is_key_down(Key::RightArrow) {
                dx += dist;
                x = -1;
            }
            if ctx.is_key_down(Key::UpArrow) {
                dy -= dist;
                y = -1;
            }
            if ctx.is_key_down(Key::DownArrow) {
                dy += dist;
                y = 1;
            }

            // TODO Better way to do this; acos and asin?
            self.facing = match (x, y) {
                (-1, -1) => Angle::degrees(135.0),
                (-1, 0) => Angle::degrees(180.0),
                (-1, 1) => Angle::degrees(225.0),
                (0, -1) => Angle::degrees(90.0),
                (0, 1) => Angle::degrees(270.0),
                (1, -1) => Angle::degrees(45.0),
                (1, 0) => Angle::degrees(0.0),
                (1, 1) => Angle::degrees(315.0),
                (0, 0) => self.facing,
                _ => unreachable!(),
            };
        }

        (dx, dy)
    }
}
