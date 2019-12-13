use crate::game::{State, Transition, WizardState};
use crate::sandbox::{GameplayMode, SandboxMode};
use crate::ui::UI;
use ezgui::layout::Widget;
use ezgui::{
    hotkey, layout, Button, Color, DrawBoth, EventCtx, EventLoopMode, GeomBatch, GfxCtx,
    HorizontalAlignment, Key, Line, RewriteColor, ScreenPt, ScreenRectangle, Slider, Text,
    VerticalAlignment, Wizard,
};
use geom::{Distance, Duration, Line, Polygon, Pt2D, Time};
use std::time::Instant;

// Layouting is very hardcoded right now.

const TIME_PANEL_WIDTH: f64 = 340.0;
const TIME_PANEL_HEIGHT: f64 = 130.0;

const SPEED_PANEL_WIDTH: f64 = 650.0;
const SPEED_PANEL_HEIGHT: f64 = 50.0;

const ADJUST_SPEED_PERCENT: f64 = 0.01;

pub struct SpeedControls {
    time_panel: TimePanel,
    top_left: ScreenPt,

    draw_fixed: DrawBoth,
    resume_btn: Button,
    pause_btn: Button,
    jump_to_time_btn: Button,
    small_step_btn: Button,
    large_step_btn: Button,
    reset_btn: Button,
    speed_slider: Slider,
    slow_down_btn: Button,
    speed_up_btn: Button,

    state: SpeedState,
    speed_cap: f64,
}

enum SpeedState {
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
        let draw_fixed = {
            let mut batch = GeomBatch::new();
            let mut txt = Vec::new();

            // Speed panel
            batch.push(
                Color::hex("#4C4C4C"),
                Polygon::rounded_rectangle(
                    Distance::meters(SPEED_PANEL_WIDTH),
                    Distance::meters(SPEED_PANEL_HEIGHT),
                    Distance::meters(5.0),
                ),
            );

            // Slider background
            // TODO Figure these out automatically...
            batch.push(
                Color::grey(0.5),
                Polygon::rounded_rectangle(
                    Distance::meters(310.0),
                    Distance::meters(22.0),
                    Distance::meters(5.0),
                )
                .translate(330.0, 10.0),
            );
            txt.push((
                Text::from(Line("speed").size(14).roboto()).no_bg(),
                ScreenPt::new(330.0, 15.0),
            ));

            DrawBoth::new(ctx, batch, txt)
        };

        let top_left = ScreenPt::new(
            (ctx.canvas.window_width - SPEED_PANEL_WIDTH) / 2.0,
            ctx.canvas.window_height - SPEED_PANEL_HEIGHT - 50.0,
        );

        let mut resume_btn = Button::rectangle_svg(
            "assets/speed/resume.svg",
            "resume",
            hotkey(Key::Space),
            RewriteColor::ChangeAll(Color::ORANGE),
            ctx,
        );
        let pause_btn = Button::rectangle_svg(
            "assets/speed/pause.svg",
            "pause",
            hotkey(Key::Space),
            RewriteColor::ChangeAll(Color::ORANGE),
            ctx,
        )
        .at(top_left);
        let mut jump_to_time_btn = Button::rectangle_svg(
            "assets/speed/jump_to_time.svg",
            "jump to specific time",
            hotkey(Key::B),
            RewriteColor::ChangeAll(Color::ORANGE),
            ctx,
        );
        let mut small_step_btn = Button::text(
            Text::from(Line("+0.1s").fg(Color::WHITE).size(12)),
            Color::WHITE.alpha(0.0),
            Color::ORANGE,
            hotkey(Key::M),
            "step forwards 0.1 seconds",
            ctx,
        );
        let mut large_step_btn = Button::rectangle_svg(
            "assets/speed/large_step.svg",
            "step forwards 1 hour",
            hotkey(Key::N),
            RewriteColor::ChangeAll(Color::ORANGE),
            ctx,
        );
        let mut reset_btn = Button::text(
            Text::from(Line("reset").fg(Color::WHITE).size(12)),
            Color::WHITE.alpha(0.0),
            Color::ORANGE,
            hotkey(Key::X),
            "reset to midnight",
            ctx,
        );
        layout::stack_horizontally(
            ScreenPt::new(top_left.x, top_left.y + 5.0),
            10.0,
            vec![
                &mut resume_btn,
                &mut jump_to_time_btn,
                &mut small_step_btn,
                &mut large_step_btn,
                &mut reset_btn,
            ],
        );

        // 10 sim minutes / real second normally, or 1 sim hour / real second for dev mode
        let speed_cap: f64 = if dev_mode { 3600.0 } else { 600.0 };
        let mut speed_slider = Slider::new(157.0, 10.0);
        // Start with speed=1.0
        speed_slider.set_percent(ctx, (speed_cap / 1.0).powf(-1.0 / std::f64::consts::E));
        speed_slider.set_pos(ScreenPt::new(top_left.x + 350.0, top_left.y + 15.0));

        let slow_down_btn = Button::rectangle_svg(
            "assets/speed/slow_down.svg",
            "slow down",
            hotkey(Key::LeftBracket),
            RewriteColor::ChangeAll(Color::ORANGE),
            ctx,
        )
        .at(ScreenPt::new(top_left.x + 500.0, top_left.y + 10.0));
        let speed_up_btn = Button::rectangle_svg(
            "assets/speed/speed_up.svg",
            "speed up",
            hotkey(Key::RightBracket),
            RewriteColor::ChangeAll(Color::ORANGE),
            ctx,
        )
        .at(ScreenPt::new(top_left.x + 600.0, top_left.y + 10.0));

        SpeedControls {
            time_panel: TimePanel::new(ctx),
            top_left,

            draw_fixed,
            resume_btn,
            pause_btn,
            jump_to_time_btn,
            small_step_btn,
            large_step_btn,
            reset_btn,
            speed_slider,
            slow_down_btn,
            speed_up_btn,

            state: SpeedState::Paused,
            speed_cap,
        }
    }

    pub fn event(
        &mut self,
        ctx: &mut EventCtx,
        ui: &mut UI,
        gameplay: &GameplayMode,
    ) -> Option<Transition> {
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
            SpeedState::Paused => {
                self.resume_btn.event(ctx);
                if self.resume_btn.clicked() {
                    let now = Instant::now();
                    self.state = SpeedState::Running {
                        last_step: now,
                        speed_description: "...".to_string(),
                        last_measurement: now,
                        last_measurement_sim: ui.primary.sim.time(),
                    };
                    return None;
                }
            }
            SpeedState::Running {
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
            return Some(Transition::PushWithMode(
                Box::new(TimeWarpScreen {
                    target: ui.primary.sim.time() + Duration::hours(1),
                    started: Instant::now(),
                }),
                EventLoopMode::Animation,
            ));
        }

        self.jump_to_time_btn.event(ctx);
        if self.jump_to_time_btn.clicked() {
            return Some(Transition::Push(WizardState::new(Box::new(jump_to_time))));
        }

        self.reset_btn.event(ctx);
        if self.reset_btn.clicked() {
            ui.primary.clear_sim();
            return Some(Transition::Replace(Box::new(SandboxMode::new(
                ctx,
                ui,
                gameplay.clone(),
            ))));
        }

        None
    }

    pub fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        self.time_panel.draw(g, ui);

        self.draw_fixed.redraw(self.top_left, g);
        g.canvas.mark_covered_area(ScreenRectangle {
            x1: self.top_left.x,
            y1: self.top_left.y,
            x2: self.top_left.x + SPEED_PANEL_WIDTH,
            y2: self.top_left.y + SPEED_PANEL_HEIGHT,
        });

        if self.is_paused() {
            self.resume_btn.draw(g);
        } else {
            self.pause_btn.draw(g);
        }

        self.jump_to_time_btn.draw(g);

        self.small_step_btn.draw(g);
        self.large_step_btn.draw(g);
        self.reset_btn.draw(g);

        {
            self.speed_slider.draw(g);

            self.slow_down_btn.draw(g);

            // TODO Center this text
            g.draw_text_at_screenspace_topleft(
                &Text::from(
                    Line(format!("{:.1}x", self.desired_speed()))
                        .size(14)
                        .roboto(),
                )
                .no_bg(),
                ScreenPt::new(self.top_left.x + 530.0, self.top_left.y + 10.0),
            );

            self.speed_up_btn.draw(g);
        }
    }

    pub fn pause(&mut self) {
        if !self.is_paused() {
            self.state = SpeedState::Paused;
        }
    }

    pub fn is_paused(&self) -> bool {
        match self.state {
            SpeedState::Paused => true,
            SpeedState::Running { .. } => false,
        }
    }

    fn desired_speed(&self) -> f64 {
        self.speed_cap * self.speed_slider.get_percent().powf(std::f64::consts::E)
    }
}

fn jump_to_time(wiz: &mut Wizard, ctx: &mut EventCtx, ui: &mut UI) -> Option<Transition> {
    let target = wiz.wrap(ctx).input_time_slider(
        "Jump to what time?",
        ui.primary.sim.time(),
        Time::END_OF_DAY,
    )?;
    Some(Transition::ReplaceWithMode(
        Box::new(TimeWarpScreen {
            target,
            started: Instant::now(),
        }),
        EventLoopMode::Animation,
    ))
}

// Display a nicer screen for jumping forwards in time, allowing cancellation.
pub struct TimeWarpScreen {
    target: Time,
    started: Instant,
}

impl State for TimeWarpScreen {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        if ctx.input.new_was_pressed(hotkey(Key::Escape).unwrap()) {
            return Transition::Pop;
        }
        if ctx.input.nonblocking_is_update_event() {
            ctx.input.use_update_event();
            ui.primary.sim.time_limited_step(
                &ui.primary.map,
                self.target - ui.primary.sim.time(),
                Duration::seconds(0.1),
            );
            // TODO secondary for a/b test mode
        }
        if ui.primary.sim.time() == self.target {
            return Transition::Pop;
        }

        Transition::KeepWithMode(EventLoopMode::Animation)
    }

    fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        // TODO Instead display base speed controls, some indication of target time and ability to
        // cancel
        let mut txt = Text::prompt("Warping through time...");
        txt.add(Line(format!(
            "Simulating until it's {}",
            self.target.ampm_tostring()
        )));
        txt.add(Line(format!(
            "It's currently {}",
            ui.primary.sim.time().ampm_tostring()
        )));
        txt.add(Line(format!(
            "Have been simulating for {}",
            Duration::realtime_elapsed(self.started)
        )));
        txt.add(Line(format!("Press ESCAPE to stop now")));
        g.draw_blocking_text(
            &txt,
            (HorizontalAlignment::Center, VerticalAlignment::Center),
        );
    }
}

struct TimePanel {
    draw_fixed: DrawBoth,
}

impl TimePanel {
    fn new(ctx: &mut EventCtx) -> TimePanel {
        let mut batch = GeomBatch::new();
        let mut txt = Vec::new();

        // Time panel background
        batch.push(
            Color::hex("#4C4C4C"),
            Polygon::rounded_rectangle(
                Distance::meters(TIME_PANEL_WIDTH),
                Distance::meters(TIME_PANEL_HEIGHT),
                Distance::meters(5.0),
            ),
        );

        txt.push((
            Text::from(Line("00:00").size(12).roboto()).no_bg(),
            ScreenPt::new(25.0, 80.0),
        ));
        batch.add_svg("assets/speed/sunrise.svg", 94.0, 80.0);
        txt.push((
            Text::from(Line("12:00").size(12).roboto()).no_bg(),
            ScreenPt::new(153.0, 80.0),
        ));
        batch.add_svg("assets/speed/sunset.svg", 220.0, 80.0);
        txt.push((
            Text::from(Line("24:00").size(12).roboto()).no_bg(),
            ScreenPt::new(280.0, 80.0),
        ));

        TimePanel {
            draw_fixed: DrawBoth::new(ctx, batch, txt),
        }
    }

    fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        self.draw_fixed.redraw(ScreenPt::new(0.0, 0.0), g);
        g.canvas.mark_covered_area(ScreenRectangle {
            x1: 0.0,
            y1: 0.0,
            x2: TIME_PANEL_WIDTH,
            y2: TIME_PANEL_HEIGHT,
        });

        g.draw_text_at_screenspace_topleft(
            &Text::from(Line(ui.primary.sim.time().ampm_tostring()).size(30)).no_bg(),
            ScreenPt::new(24.0, 10.0),
        );

        {
            let x1 = 24.0;
            let y1 = 65.0;
            let percent = ui.primary.sim.time().to_percent(Time::END_OF_DAY);
            let width = 287.0;
            let height = Distance::meters(15.0);

            g.fork_screenspace();
            // TODO rounded
            g.draw_polygon(
                Color::WHITE,
                &Line::new(Pt2D::new(x1, y1), Pt2D::new(x1 + width, y1)).make_polygons(height),
            );
            if let Some(l) = Line::maybe_new(Pt2D::new(x1, y1), Pt2D::new(x1 + percent * width, y1))
            {
                g.draw_polygon(Color::grey(0.5), &l.make_polygons(height));
            }
            g.unfork();
        }
    }
}
