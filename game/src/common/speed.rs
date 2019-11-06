use abstutil::elapsed_seconds;
use ezgui::layout::Widget;
use ezgui::{
    hotkey, Button, Color, Drawable, EventCtx, GeomBatch, GfxCtx, Key, Line, ScreenPt,
    ScreenRectangle, Slider, Text,
};
use geom::{Distance, Duration, Polygon, Pt2D};
use std::time::Instant;

const PANEL_RECT: ScreenRectangle = ScreenRectangle {
    x1: 0.0,
    y1: 0.0,
    x2: 350.0,
    y2: 150.0,
};

const ADJUST_SPEED_PERCENT: f64 = 0.01;
// TODO hardcoded cap for now... 10 sim minutes / real second
const SPEED_CAP: f64 = 600.0;

pub struct SpeedControls {
    slider: Slider,
    state: State,

    panel_bg: Drawable,
    resume_btn: Button,
    pause_btn: Button,
    slow_down_btn: Button,
    speed_up_btn: Button,

    pub small_step_btn: Option<Button>,
    pub large_step_btn: Option<Button>,
    pub jump_to_time_btn: Option<Button>,
}

enum State {
    Paused,
    Running {
        last_step: Instant,
        speed_description: String,
        last_measurement: Instant,
        last_measurement_sim: Duration,
    },
}

impl SpeedControls {
    pub fn new(ctx: &mut EventCtx, step_controls: bool) -> SpeedControls {
        let mut panel_bg = GeomBatch::new();
        panel_bg.push(
            Color::grey(0.3),
            Polygon::rectangle_topleft(
                Pt2D::new(PANEL_RECT.x1, PANEL_RECT.y1),
                Distance::meters(PANEL_RECT.x2 - PANEL_RECT.x1),
                Distance::meters(PANEL_RECT.y2 - PANEL_RECT.y1),
            ),
        );

        let mut resume_btn = Button::icon_btn(
            "assets/ui/resume.png",
            50.0,
            "resume",
            hotkey(Key::Space),
            ctx,
        );
        resume_btn.set_pos(ScreenPt::new(0.0, 0.0), 0.0);
        let mut pause_btn = Button::icon_btn(
            "assets/ui/pause.png",
            50.0,
            "pause",
            hotkey(Key::Space),
            ctx,
        );
        pause_btn.set_pos(ScreenPt::new(0.0, 0.0), 0.0);

        let mut slow_down_btn = Button::icon_btn(
            "assets/ui/slow_down.png",
            25.0,
            "slow down",
            hotkey(Key::LeftBracket),
            ctx,
        );
        slow_down_btn.set_pos(ScreenPt::new(100.0, 50.0), 0.0);
        let mut speed_up_btn = Button::icon_btn(
            "assets/ui/speed_up.png",
            25.0,
            "speed up",
            hotkey(Key::RightBracket),
            ctx,
        );
        speed_up_btn.set_pos(ScreenPt::new(150.0, 50.0), 0.0);

        let mut slider = Slider::new();
        // Start with speed=1.0
        slider.set_percent(ctx, (SPEED_CAP / 5.0).powf(-1.0 / std::f64::consts::E));
        slider.set_pos(ScreenPt::new(0.0, 100.0), 150.0);

        let (small_step_btn, large_step_btn, jump_to_time_btn) = if step_controls {
            let mut small = Button::icon_btn(
                "assets/ui/small_step.png",
                25.0,
                "step forwards 0.1s",
                hotkey(Key::M),
                ctx,
            );
            small.set_pos(ScreenPt::new(200.0, 50.0), 0.0);

            let mut large = Button::icon_btn(
                "assets/ui/large_step.png",
                25.0,
                "step forwards 10 mins",
                hotkey(Key::N),
                ctx,
            );
            large.set_pos(ScreenPt::new(250.0, 50.0), 0.0);

            let mut jump = Button::icon_btn(
                "assets/ui/jump_to_time.png",
                25.0,
                "jump to specific time",
                hotkey(Key::B),
                ctx,
            );
            jump.set_pos(ScreenPt::new(300.0, 50.0), 0.0);

            (Some(small), Some(large), Some(jump))
        } else {
            (None, None, None)
        };

        SpeedControls {
            slider,
            state: State::Paused,

            panel_bg: ctx.prerender.upload(panel_bg),
            resume_btn,
            pause_btn,
            slow_down_btn,
            speed_up_btn,
            small_step_btn,
            large_step_btn,
            jump_to_time_btn,
        }
    }

    // Returns the amount of simulation time to step, if running.
    pub fn event(&mut self, ctx: &mut EventCtx, current_sim_time: Duration) -> Option<Duration> {
        self.slow_down_btn.event(ctx);
        self.speed_up_btn.event(ctx);

        let desired_speed = self.desired_speed();
        if self.speed_up_btn.clicked() && desired_speed != SPEED_CAP {
            self.slider.set_percent(
                ctx,
                (self.slider.get_percent() + ADJUST_SPEED_PERCENT).min(1.0),
            );
        } else if self.slow_down_btn.clicked() && desired_speed != 0.0 {
            self.slider.set_percent(
                ctx,
                (self.slider.get_percent() - ADJUST_SPEED_PERCENT).max(0.0),
            );
        } else if self.slider.event(ctx) {
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
                        last_measurement_sim: current_sim_time,
                    };
                    // Sorta hack to trigger EventLoopMode::Animation.
                    return Some(Duration::ZERO);
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
                    let dt = Duration::seconds(elapsed_seconds(*last_step)) * desired_speed;
                    *last_step = Instant::now();

                    let dt_descr = Duration::seconds(elapsed_seconds(*last_measurement));
                    if dt_descr >= Duration::seconds(1.0) {
                        *speed_description = format!(
                            "{:.2}x",
                            (current_sim_time - *last_measurement_sim) / dt_descr
                        );
                        *last_measurement = *last_step;
                        *last_measurement_sim = current_sim_time;
                    }
                    return Some(dt);
                }
            }
        }
        None
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        if let State::Running {
            ref speed_description,
            ..
        } = self.state
        {
            g.draw_text_at_screenspace_topleft(
                &Text::from(Line(format!("Speed: {}", speed_description))),
                ScreenPt::new(100.0, 0.0),
            );
        } else {
            g.draw_text_at_screenspace_topleft(
                &Text::from(Line("Speed")),
                ScreenPt::new(100.0, 0.0),
            );
        }
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
            &Text::from(Line(format!("{:.2}x", self.desired_speed()))),
            ScreenPt::new(150.0, 100.0),
        );

        if let Some(ref btn) = self.small_step_btn {
            btn.draw(g);
        }
        if let Some(ref btn) = self.large_step_btn {
            btn.draw(g);
        }
        if let Some(ref btn) = self.jump_to_time_btn {
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

    fn desired_speed(&self) -> f64 {
        SPEED_CAP * self.slider.get_percent().powf(std::f64::consts::E)
    }
}
