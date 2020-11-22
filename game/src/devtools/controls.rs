use geom::{Angle, Circle, Distance, Pt2D, Speed};
use widgetry::{
    Btn, Checkbox, Color, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Panel, State,
    UpdateType, VerticalAlignment, Widget,
};

use crate::app::App;
use crate::game::Transition;

pub struct Experiment {
    panel: Panel,
    controls: Box<dyn Controller>,
    sleigh: Pt2D,
}

impl Experiment {
    pub fn new(ctx: &mut EventCtx) -> Box<dyn State<App>> {
        Box::new(Experiment {
            panel: Panel::new(Widget::col(vec![
                Widget::row(vec![
                    Line("Experiment").small_heading().draw(ctx),
                    Btn::close(ctx),
                ]),
                Checkbox::toggle(ctx, "control type", "rotate", "instant", Key::Tab, false),
            ]))
            .aligned(HorizontalAlignment::Right, VerticalAlignment::Top)
            .build(ctx),
            controls: Box::new(InstantController::new(Speed::miles_per_hour(30.0))),
            sleigh: Pt2D::new(0.0, 0.0),
        })
    }
}

impl State<App> for Experiment {
    fn event(&mut self, ctx: &mut EventCtx, _: &mut App) -> Transition {
        let (dx, dy) = self.controls.displacement(ctx);
        self.sleigh = self.sleigh.offset(dx, dy);
        ctx.canvas.center_on_map_pt(self.sleigh);

        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                _ => unreachable!(),
            },
            Outcome::Changed => {
                self.controls = if self.panel.is_checked("control type") {
                    Box::new(RotateController::new(Speed::miles_per_hour(30.0)))
                } else {
                    Box::new(InstantController::new(Speed::miles_per_hour(30.0)))
                };
            }
            _ => {}
        }

        ctx.request_update(UpdateType::Game);
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.panel.draw(g);

        g.draw_polygon(
            Color::RED,
            Circle::new(self.sleigh, Distance::meters(5.0)).to_polygon(),
        );
    }
}

trait Controller {
    fn displacement(&mut self, ctx: &mut EventCtx) -> (f64, f64);
}

struct InstantController {
    left_key_pressed: bool,
    right_key_pressed: bool,
    up_key_pressed: bool,
    down_key_pressed: bool,

    speed: Speed,
}

impl InstantController {
    fn new(speed: Speed) -> InstantController {
        InstantController {
            left_key_pressed: false,
            right_key_pressed: false,
            up_key_pressed: false,
            down_key_pressed: false,

            // TODO Hack
            speed: 5.0 * speed,
        }
    }
}

impl Controller for InstantController {
    fn displacement(&mut self, ctx: &mut EventCtx) -> (f64, f64) {
        if ctx.input.pressed(Key::LeftArrow) {
            self.left_key_pressed = true;
        }
        if ctx.input.key_released(Key::LeftArrow) {
            self.left_key_pressed = false;
        }
        if ctx.input.pressed(Key::RightArrow) {
            self.right_key_pressed = true;
        }
        if ctx.input.key_released(Key::RightArrow) {
            self.right_key_pressed = false;
        }
        if ctx.input.pressed(Key::UpArrow) {
            self.up_key_pressed = true;
        }
        if ctx.input.key_released(Key::UpArrow) {
            self.up_key_pressed = false;
        }
        if ctx.input.pressed(Key::DownArrow) {
            self.down_key_pressed = true;
        }
        if ctx.input.key_released(Key::DownArrow) {
            self.down_key_pressed = false;
        }

        let mut dx = 0.0;
        let mut dy = 0.0;

        if let Some(dt) = ctx.input.nonblocking_is_update_event() {
            ctx.input.use_update_event();

            let dist = (dt * self.speed).inner_meters();
            if self.left_key_pressed {
                dx -= dist;
            }
            if self.right_key_pressed {
                dx += dist;
            }
            if self.up_key_pressed {
                dy -= dist;
            }
            if self.down_key_pressed {
                dy += dist;
            }
        }

        (dx, dy)
    }
}

struct RotateController {
    left_key_pressed: bool,
    right_key_pressed: bool,
    up_key_pressed: bool,

    angle: Angle,
    rot_speed_degrees: f64,
    fwd_speed: Speed,
}

impl RotateController {
    fn new(fwd_speed: Speed) -> RotateController {
        RotateController {
            left_key_pressed: false,
            right_key_pressed: false,
            up_key_pressed: false,

            angle: Angle::ZERO,
            rot_speed_degrees: 100.0,
            // TODO Hack
            fwd_speed: 5.0 * fwd_speed,
        }
    }
}

impl Controller for RotateController {
    fn displacement(&mut self, ctx: &mut EventCtx) -> (f64, f64) {
        if ctx.input.pressed(Key::LeftArrow) {
            self.left_key_pressed = true;
        }
        if ctx.input.key_released(Key::LeftArrow) {
            self.left_key_pressed = false;
        }
        if ctx.input.pressed(Key::RightArrow) {
            self.right_key_pressed = true;
        }
        if ctx.input.key_released(Key::RightArrow) {
            self.right_key_pressed = false;
        }
        if ctx.input.pressed(Key::UpArrow) {
            self.up_key_pressed = true;
        }
        if ctx.input.key_released(Key::UpArrow) {
            self.up_key_pressed = false;
        }

        let mut dx = 0.0;
        let mut dy = 0.0;

        if let Some(dt) = ctx.input.nonblocking_is_update_event() {
            ctx.input.use_update_event();

            if self.left_key_pressed {
                self.angle = self
                    .angle
                    .rotate_degs(-self.rot_speed_degrees * dt.inner_seconds());
            }
            if self.right_key_pressed {
                self.angle = self
                    .angle
                    .rotate_degs(self.rot_speed_degrees * dt.inner_seconds());
            }

            if self.up_key_pressed {
                let dist = dt * self.fwd_speed;
                let pt = Pt2D::new(0.0, 0.0).project_away(dist, self.angle);
                dx = pt.x();
                dy = pt.y();
            }
        }

        (dx, dy)
    }
}
