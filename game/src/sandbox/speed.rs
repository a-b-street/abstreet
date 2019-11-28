use crate::game::{Transition, WizardState};
use crate::ui::UI;
use ezgui::layout::Widget;
use ezgui::{
    hotkey, Button, Color, Drawable, EventCtx, GeomBatch, GfxCtx, JustDraw, Key, Line, ScreenPt,
    ScreenRectangle, Slider, Text, Wizard,
};
use geom::{Distance, Duration, Line, Polygon, Pt2D, Time};
use std::time::Instant;

// TODO Everything in here is terrible. Layouting sucks.

const PANEL_RECT: ScreenRectangle = ScreenRectangle {
    x1: 0.0,
    y1: 0.0,
    x2: 460.0,
    y2: 200.0,
};

const ADJUST_SPEED_PERCENT: f64 = 0.01;

pub struct SpeedControls {
    speed_slider: Slider,
    state: State,
    speed_cap: f64,

    panel_bg: Drawable,
    resume_btn: Button,
    pause_btn: Button,
    slow_down_btn: Button,
    speed_up_btn: Button,

    small_step_btn: Button,
    large_step_btn: Button,
    jump_to_time_btn: Button,

    sunrise: JustDraw,
    sunset: JustDraw,
}

enum State {
    Paused,
    Running {
        last_step: Instant,
        speed_description: String,
        last_measurement: Instant,
        last_measurement_sim: Time,
    },
}

impl SpeedControls {
    pub fn new(ctx: &mut EventCtx, dev_mode: bool) -> SpeedControls {
        let mut panel_bg = GeomBatch::new();
        panel_bg.push(
            Color::grey(0.3),
            Polygon::rectangle_topleft(
                Pt2D::new(PANEL_RECT.x1, PANEL_RECT.y1),
                Distance::meters(PANEL_RECT.x2 - PANEL_RECT.x1),
                Distance::meters(PANEL_RECT.y2 - PANEL_RECT.y1),
            ),
        );

        let resume_btn = Button::rectangle_img_no_bg(
            "assets/ui/resume.png",
            "resume",
            hotkey(Key::Space),
            ctx,
        )
        .at(ScreenPt::new(10.0, 10.0));
        let pause_btn = Button::rectangle_img_no_bg(
            "assets/ui/pause.png",
            "pause",
            hotkey(Key::Space),
            ctx,
        )
        .at(ScreenPt::new(10.0, 10.0));

        let jump_to_time_btn = Button::rectangle_img_no_bg(
            "assets/ui/jump_to_time.png",
            "jump to specific time",
            hotkey(Key::B),
            ctx,
        )
        .at(ScreenPt::new(405.0, 10.0));

        let small_step_btn = Button::rectangle_img_no_bg(
            "assets/ui/small_step.png",
            "step forwards 0.1s",
            hotkey(Key::M),
            ctx,
        )
        .at(ScreenPt::new(380.0, 70.0));

        let large_step_btn = Button::rectangle_img_no_bg(
            "assets/ui/large_step.png",
            "step forwards 10 mins",
            hotkey(Key::N),
            ctx,
        )
        .at(ScreenPt::new(380.0, 100.0));

        let mut sunrise = JustDraw::image("assets/ui/sunrise.png", ctx);
        sunrise.set_pos(ScreenPt::new(92.0, 120.0));
        let mut sunset = JustDraw::image("assets/ui/sunset.png", ctx);
        sunset.set_pos(ScreenPt::new(254.0, 120.0));

        // 10 sim minutes / real second normally, or 1 sim hour / real second for dev mode
        let speed_cap: f64 = if dev_mode { 3600.0 } else { 600.0 };
        let mut speed_slider = Slider::new(270.0);
        // Start with speed=1.0
        speed_slider.set_percent(ctx, (speed_cap / 1.0).powf(-1.0 / std::f64::consts::E));
        speed_slider.set_pos(ScreenPt::new(50.0, 150.0));

        let slow_down_btn = Button::rectangle_img_no_bg(
            "assets/ui/slow_down.png",
            "slow down",
            hotkey(Key::LeftBracket),
            ctx,
        )
        .at(ScreenPt::new(290.0, 170.0));
        let speed_up_btn = Button::rectangle_img_no_bg(
            "assets/ui/speed_up.png",
            "speed up",
            hotkey(Key::RightBracket),
            ctx,
        )
        .at(ScreenPt::new(425.0, 160.0));

        SpeedControls {
            speed_slider,
            state: State::Paused,
            speed_cap,

            panel_bg: panel_bg.upload(ctx),
            resume_btn,
            pause_btn,
            slow_down_btn,
            speed_up_btn,
            small_step_btn,
            large_step_btn,
            jump_to_time_btn,
            sunrise,
            sunset,
        }
    }

    pub fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Option<Transition> {
        self.slow_down_btn.event(ctx);
        self.speed_up_btn.event(ctx);

        let desired_speed = self.desired_speed();
        if self.speed_up_btn.clicked() && desired_speed != self.speed_cap {
            self.speed_slider.set_percent(
                ctx,
                (self.speed_slider.get_percent() + ADJUST_SPEED_PERCENT).min(1.0),
            );
        } else if self.slow_down_btn.clicked() && desired_speed != 0.0 {
            self.speed_slider.set_percent(
                ctx,
                (self.speed_slider.get_percent() - ADJUST_SPEED_PERCENT).max(0.0),
            );
        } else if self.speed_slider.event(ctx) {
            // Keep going
        }

        match self.state {
            State::Paused => {
                self.resume_btn.event(ctx);
                if self.resume_btn.clicked() {
                    let now = Instant::now();
                    self.state = State::Running {
                        last_step: now,
                        speed_description: "...".to_string(),
                        last_measurement: now,
                        last_measurement_sim: ui.primary.sim.time(),
                    };
                    return None;
                }
            }
            State::Running {
                ref mut last_step,
                ref mut speed_description,
                ref mut last_measurement,
                ref mut last_measurement_sim,
            } => {
                self.pause_btn.event(ctx);
                if self.pause_btn.clicked() {
                    self.pause();
                } else if ctx.input.nonblocking_is_update_event() {
                    ctx.input.use_update_event();
                    let dt = Duration::realtime_elapsed(*last_step) * desired_speed;
                    *last_step = Instant::now();

                    let dt_descr = Duration::realtime_elapsed(*last_measurement);
                    if dt_descr >= Duration::seconds(1.0) {
                        *speed_description = format!(
                            "{:.2}x",
                            (ui.primary.sim.time() - *last_measurement_sim) / dt_descr
                        );
                        *last_measurement = *last_step;
                        *last_measurement_sim = ui.primary.sim.time();
                    }
                    // If speed is too high, don't be unresponsive for too long.
                    // TODO This should probably match the ezgui framerate.
                    ui.primary
                        .sim
                        .time_limited_step(&ui.primary.map, dt, Duration::seconds(0.1));
                    ui.recalculate_current_selection(ctx);
                }
            }
        }

        self.small_step_btn.event(ctx);
        if self.small_step_btn.clicked() {
            ui.primary.sim.step(&ui.primary.map, Duration::seconds(0.1));
            if let Some(ref mut s) = ui.secondary {
                s.sim.step(&s.map, Duration::seconds(0.1));
            }
            ui.recalculate_current_selection(ctx);
        }

        self.large_step_btn.event(ctx);
        if self.large_step_btn.clicked() {
            ctx.loading_screen("step forwards 10 minutes", |_, mut timer| {
                ui.primary
                    .sim
                    .timed_step(&ui.primary.map, Duration::minutes(10), &mut timer);
                if let Some(ref mut s) = ui.secondary {
                    s.sim.timed_step(&s.map, Duration::minutes(10), &mut timer);
                }
            });
            ui.recalculate_current_selection(ctx);
        }

        self.jump_to_time_btn.event(ctx);
        if self.jump_to_time_btn.clicked() {
            return Some(Transition::Push(WizardState::new(Box::new(jump_to_time))));
        }

        None
    }

    pub fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        g.fork_screenspace();
        g.redraw(&self.panel_bg);
        g.canvas.mark_covered_area(PANEL_RECT);
        g.unfork();

        // Row 1

        if self.is_paused() {
            self.resume_btn.draw(g);
        } else {
            self.pause_btn.draw(g);
        }

        g.draw_text_at_screenspace_topleft(
            &Text::from(Line(ui.primary.sim.time().ampm_tostring()).size(40)).no_bg(),
            ScreenPt::new(90.0, 15.0),
        );

        self.jump_to_time_btn.draw(g);

        // Row 2

        // TODO Actual slider
        {
            let y1 = 90.0;
            let percent = ui.primary.sim.time().to_percent(Time::END_OF_DAY);
            let width = 350.0;
            let height = Distance::meters(30.0);
            g.fork_screenspace();
            // TODO rounded
            g.draw_polygon(
                Color::WHITE,
                &Line::new(Pt2D::new(10.0, y1), Pt2D::new(10.0 + width, y1)).make_polygons(height),
            );
            if let Some(l) =
                Line::maybe_new(Pt2D::new(10.0, y1), Pt2D::new(10.0 + percent * width, y1))
            {
                g.draw_polygon(Color::grey(0.5), &l.make_polygons(height));
            }
            g.unfork();
        }

        self.small_step_btn.draw(g);
        self.large_step_btn.draw(g);

        // Row 3

        {
            // time slider is x = 10 to 360
            let y1 = 120.0;
            g.draw_text_at_screenspace_topleft(
                &Text::from(Line("00:00").size(18)).no_bg(),
                ScreenPt::new(10.0, y1),
            );
            self.sunrise.draw(g);
            g.draw_text_at_screenspace_topleft(
                &Text::from(Line("12:00").size(18)).no_bg(),
                ScreenPt::new(175.0, y1),
            );
            self.sunset.draw(g);
            g.draw_text_at_screenspace_topleft(
                &Text::from(Line("24:00").size(18)).no_bg(),
                ScreenPt::new(360.0, y1),
            );
        }

        // Row 4

        {
            let y1 = 150.0;
            g.fork_screenspace();
            g.draw_polygon(
                Color::grey(0.5),
                &Polygon::rounded_rectangle(
                    Distance::meters(0.95 * (PANEL_RECT.x2 - PANEL_RECT.x1)),
                    Distance::meters(40.0),
                    Distance::meters(5.0),
                )
                .translate(10.0, y1),
            );
            g.unfork();

            g.draw_text_at_screenspace_topleft(
                &Text::from(Line("speed").size(25)).no_bg(),
                ScreenPt::new(10.0, y1),
            );

            self.speed_slider.draw(g);

            self.slow_down_btn.draw(g);

            // TODO Center this text
            g.draw_text_at_screenspace_topleft(
                &Text::from(Line(format!("{:.2}x", self.desired_speed())).size(25)).no_bg(),
                ScreenPt::new(320.0, y1 + 5.0),
            );

            self.speed_up_btn.draw(g);
        }
    }

    pub fn pause(&mut self) {
        if !self.is_paused() {
            self.state = State::Paused;
        }
    }

    pub fn is_paused(&self) -> bool {
        match self.state {
            State::Paused => true,
            State::Running { .. } => false,
        }
    }

    fn desired_speed(&self) -> f64 {
        self.speed_cap * self.speed_slider.get_percent().powf(std::f64::consts::E)
    }
}

fn jump_to_time(wiz: &mut Wizard, ctx: &mut EventCtx, ui: &mut UI) -> Option<Transition> {
    let t = wiz.wrap(ctx).input_time_slider(
        "Jump to what time?",
        ui.primary.sim.time(),
        Time::END_OF_DAY,
    )?;
    let dt = t - ui.primary.sim.time();
    ctx.loading_screen(&format!("step forwards {}", dt), |_, mut timer| {
        ui.primary.sim.timed_step(&ui.primary.map, dt, &mut timer);
        if let Some(ref mut s) = ui.secondary {
            s.sim.timed_step(&s.map, dt, &mut timer);
        }
    });
    Some(Transition::Pop)
}
