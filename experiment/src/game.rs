use geom::{Angle, Circle, Distance, Pt2D, Speed};
use map_gui::common::CityPicker;
use map_gui::helpers::nice_map_name;
use map_gui::SimpleApp;
use widgetry::{
    lctrl, Btn, Checkbox, Color, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Panel,
    State, Transition, UpdateType, VerticalAlignment, Widget,
};

pub struct Game {
    panel: Panel,
    controls: Box<dyn Controller>,
    sleigh: Pt2D,
}

impl Game {
    pub fn new(ctx: &mut EventCtx, app: &SimpleApp) -> Box<dyn State<SimpleApp>> {
        Box::new(Game {
            panel: Panel::new(Widget::col(vec![
                Widget::row(vec![
                    Line("Experiment").small_heading().draw(ctx),
                    Btn::close(ctx),
                ]),
                Checkbox::toggle(ctx, "control type", "rotate", "instant", Key::Tab, false),
                Widget::row(vec![Btn::pop_up(
                    ctx,
                    Some(nice_map_name(app.map.get_name())),
                )
                .build(ctx, "change map", lctrl(Key::L))]),
            ]))
            .aligned(HorizontalAlignment::Right, VerticalAlignment::Top)
            .build(ctx),
            controls: Box::new(InstantController::new(Speed::miles_per_hour(30.0))),
            sleigh: Pt2D::new(0.0, 0.0),
        })
    }
}

impl State<SimpleApp> for Game {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut SimpleApp) -> Transition<SimpleApp> {
        let (dx, dy) = self.controls.displacement(ctx);
        self.sleigh = self.sleigh.offset(dx, dy);
        ctx.canvas.center_on_map_pt(self.sleigh);

        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                "change map" => {
                    return Transition::Push(CityPicker::new(
                        ctx,
                        app,
                        Box::new(|ctx, app| {
                            Transition::Multi(vec![
                                Transition::Pop,
                                Transition::Replace(Game::new(ctx, app)),
                            ])
                        }),
                    ));
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

    fn draw(&self, g: &mut GfxCtx, _: &SimpleApp) {
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
    speed: Speed,
}

impl InstantController {
    fn new(speed: Speed) -> InstantController {
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

struct RotateController {
    angle: Angle,
    rot_speed_degrees: f64,
    fwd_speed: Speed,
}

impl RotateController {
    fn new(fwd_speed: Speed) -> RotateController {
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
