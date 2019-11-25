use abstutil::elapsed_seconds;
use ezgui::layout::Widget;
use ezgui::{
    hotkey, Button, Color, Drawable, EventCtx, GeomBatch, GfxCtx, Key, Line, ScreenPt,
    ScreenRectangle, Slider, Text,
};
use geom::{Distance, Duration, Polygon, Pt2D};
use std::time::Instant;
use crate::ui::UI;

const PANEL_RECT: ScreenRectangle = ScreenRectangle {
    x1: 0.0,
    y1: 0.0,
    x2: 460.0,
    y2: 150.0,
};

const ADJUST_SPEED_PERCENT: f64 = 0.01;

pub struct SpeedControls {
    slider: Slider,
    state: State,
    speed_cap: f64,
    speed_actual: f64,

    panel_bg: Drawable,
    resume_btn: Button,
    pause_btn: Button,
    slow_down_btn: Button,
    speed_up_btn: Button,

    pub small_step_btn: Option<Button>,
    pub large_step_btn: Option<Button>,
    pub edit_time_btn: Option<Button>,
}

enum State {
    Paused,
    Running {
        last_step: Instant,
        last_measurement: Instant,
        last_measurement_sim: Duration,
    },
}

impl SpeedControls {
    pub fn new(ctx: &mut EventCtx, dev_mode: bool, step_controls: bool) -> SpeedControls {
        let mut panel_bg = GeomBatch::new();
        panel_bg.push(
            Color::grey(0.3),
            Polygon::rectangle_topleft(
                Pt2D::new(PANEL_RECT.x1, PANEL_RECT.y1),
                Distance::meters(PANEL_RECT.x2 - PANEL_RECT.x1),
                Distance::meters(PANEL_RECT.y2 - PANEL_RECT.y1),
            ),
        );

        let resume_btn = Button::icon_btn(
            "assets/ui/resume.png",
            50.0,
            "resume",
            hotkey(Key::Space),
            ctx,
        )
        .at(ScreenPt::new(0.0, 0.0));
        let pause_btn = Button::icon_btn(
            "assets/ui/pause.png",
            50.0,
            "pause",
            hotkey(Key::Space),
            ctx,
        )
        .at(ScreenPt::new(0.0, 0.0));

        let slow_down_btn = Button::icon_btn(
            "assets/ui/slow_down.png",
            25.0,
            "slow down",
            hotkey(Key::LeftBracket),
            ctx,
        )
        .at(ScreenPt::new(100.0, 100.0));
        let speed_up_btn = Button::icon_btn(
            "assets/ui/speed_up.png",
            25.0,
            "speed up",
            hotkey(Key::RightBracket),
            ctx,
        )
        .at(ScreenPt::new(280.0, 100.0));

        // 10 sim minutes / real second normally, or 1 sim hour / real second for dev mode
        let speed_cap: f64 = if dev_mode { 3600.0 } else { 600.0 };
        let mut slider = Slider::new();
        // Start with speed=1.0
        slider.set_percent(ctx, 0.0);
        slider.set_pos(ScreenPt::new(100.0, 50.0), 300.0);

        let (small_step_btn, large_step_btn, edit_time_btn) = if step_controls {
            let small = Button::icon_btn(
                "assets/ui/small_step.png",
                25.0,
                "step forwards 0.1s",
                hotkey(Key::M),
                ctx,
            )
            .at(ScreenPt::new(400.0, 50.0));

            let large = Button::icon_btn(
                "assets/ui/large_step.png",
                25.0,
                "step forwards 10 mins",
                hotkey(Key::N),
                ctx,
            )
            .at(ScreenPt::new(400.0, 90.0));

            let jump = Button::icon_btn(
                "assets/ui/edit_time.png",
                25.0,
                "jump to a specific time in the future",
                hotkey(Key::B),
                ctx,
            )
            .at(ScreenPt::new(400.0, 10.0));

            (Some(small), Some(large), Some(jump))
        } else {
            (None, None, None)
        };

        SpeedControls {
            state: State::Paused,
            speed_cap,
            speed_actual: 1.0,

            panel_bg: panel_bg.upload(ctx),
            resume_btn,
            pause_btn,
            slow_down_btn,
            speed_up_btn,
            small_step_btn,
            large_step_btn,
            edit_time_btn,
            slider,
        }
    }

    // Returns the amount of simulation time to step, if running.
    pub fn event(&mut self, ctx: &mut EventCtx, current_sim_time: Duration) -> Option<Duration> {
        self.slow_down_btn.event(ctx);
        self.speed_up_btn.event(ctx);

        if self.speed_up_btn.clicked() && self.speed_actual != self.speed_cap {
            self.speed_actual += self.speed_cap * ADJUST_SPEED_PERCENT;
        } else if self.slow_down_btn.clicked() && self.speed_actual != 0.0 {
            self.speed_actual -= self.speed_cap * ADJUST_SPEED_PERCENT;
        } else if self.slider.event(ctx) {
            //
        }

        match self.state {
            State::Paused => {
                self.resume_btn.event(ctx);
                if self.resume_btn.clicked() {
                    let now = Instant::now();
                    self.state = State::Running {
                        last_step: now,
                        last_measurement: now,
                        last_measurement_sim: current_sim_time,
                    };
                    self.slider.set_percent(ctx, current_sim_time.inner_seconds()/86400.0);
                    // Sorta hack to trigger EventLoopMode::Animation.
                    return Some(Duration::ZERO);
                }
            }
            State::Running {
                ref mut last_step,
                ref mut last_measurement,
                ref mut last_measurement_sim,
            } => {
                self.pause_btn.event(ctx);
                if self.pause_btn.clicked() {
                    self.pause();
                } else if ctx.input.nonblocking_is_update_event() {
                    ctx.input.use_update_event();
                    let mut speed_desired = self.speed_actual;
                    if speed_desired > 2.0 {
                        speed_desired -= 1.0;
                    }
                    let dt = Duration::seconds(elapsed_seconds(*last_step)) * speed_desired;
                    *last_step = Instant::now();

                    let dt_descr = Duration::seconds(elapsed_seconds(*last_measurement));
                    if dt_descr >= Duration::seconds(1.0) {
                        self.speed_actual = (current_sim_time - *last_measurement_sim) / dt_descr;
                        *last_measurement = *last_step;
                        *last_measurement_sim = current_sim_time;
                    }
                    self.slider.set_percent(ctx, current_sim_time.inner_seconds()/86400.0);
                    return Some(dt);
                }
            }
        }
        None
    }

    pub fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        g.draw_text_at_screenspace_topleft(
                &Text::from(Line(format!("{}", ui.primary.sim.time().ampm_tostring())).size(40)).no_bg(),
                ScreenPt::new(110.0, 10.0),
        );
        g.fork_screenspace();
        g.redraw(&self.panel_bg);
        g.canvas.mark_covered_area(PANEL_RECT);
        g.unfork();
        if self.is_paused() {
            self.resume_btn.draw(g);
        } else {
            self.pause_btn.draw(g);
        }
        self.slow_down_btn.draw(g);
        self.speed_up_btn.draw(g);
        self.slider.draw(g);
        g.draw_text_at_screenspace_topleft(
            &Text::from(Line(format!("{:.1}x", self.speed_actual))).no_bg(),
            ScreenPt::new(170.0, 110.0),
        );

        /* todo add sunrise and sunset */
        g.draw_text_at_screenspace_topleft(
            &Text::from(Line("00:00").size(20)).no_bg(),
            ScreenPt::new(90.0, 80.0),
        );
        g.draw_text_at_screenspace_topleft(
            &Text::from(Line("12:00").size(20)).no_bg(),
            ScreenPt::new(220.0, 80.0),
        );
        g.draw_text_at_screenspace_topleft(
            &Text::from(Line("24:00").size(20)).no_bg(),
            ScreenPt::new(350.0, 80.0),
        );

        if let Some(ref btn) = self.small_step_btn {
            btn.draw(g);
        }
        if let Some(ref btn) = self.large_step_btn {
            btn.draw(g);
        }
        if let Some(ref btn) = self.edit_time_btn {
            btn.draw(g);
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
}
