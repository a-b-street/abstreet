use crate::game::{State, Transition, WizardState};
use crate::managed::{Composite, Outcome};
use crate::ui::UI;
use ezgui::{
    hotkey, Button, Color, EventCtx, EventLoopMode, GeomBatch, GfxCtx, HorizontalAlignment, Key,
    Line, ManagedWidget, RewriteColor, Slider, Text, VerticalAlignment, Wizard,
};
use geom::{Distance, Duration, Line, Pt2D, Time};
use std::time::Instant;

const ADJUST_SPEED_PERCENT: f64 = 0.01;

pub struct SpeedControls {
    composite: Composite,

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
    fn make_panel(ctx: &EventCtx, paused: bool, actual_speed: &str, slider: Slider) -> Composite {
        let mut row = Vec::new();
        if paused {
            row.push(ManagedWidget::btn(Button::rectangle_svg(
                "assets/speed/resume.svg",
                "resume",
                hotkey(Key::Space),
                RewriteColor::ChangeAll(Color::ORANGE),
                ctx,
            )));
        } else {
            row.push(ManagedWidget::btn(Button::rectangle_svg(
                "assets/speed/pause.svg",
                "pause",
                hotkey(Key::Space),
                RewriteColor::ChangeAll(Color::ORANGE),
                ctx,
            )));
        }
        row.extend(vec![
            ManagedWidget::btn(Button::rectangle_svg(
                "assets/speed/jump_to_time.svg",
                "jump to specific time",
                hotkey(Key::B),
                RewriteColor::ChangeAll(Color::ORANGE),
                ctx,
            )),
            ManagedWidget::btn(Button::text(
                Text::from(Line("+0.1s").fg(Color::WHITE).size(12)),
                Color::grey(0.6),
                Color::ORANGE,
                hotkey(Key::M),
                "step forwards 0.1 seconds",
                ctx,
            )),
            ManagedWidget::btn(Button::text(
                Text::from(Line("+1h").fg(Color::WHITE).size(12)),
                Color::grey(0.6),
                Color::ORANGE,
                hotkey(Key::M),
                "step forwards 1 hour",
                ctx,
            )),
            ManagedWidget::btn(Button::text(
                Text::from(Line("reset").fg(Color::WHITE).size(12)),
                Color::grey(0.6),
                Color::ORANGE,
                hotkey(Key::X),
                "reset to midnight",
                ctx,
            )),
        ]);

        row.push(
            ManagedWidget::row(
                vec![
                    ManagedWidget::draw_text(ctx, Text::from(Line("speed").size(14).roboto())),
                    ManagedWidget::slider("speed"),
                    ManagedWidget::btn(Button::rectangle_svg(
                        "assets/speed/slow_down.svg",
                        "slow down",
                        hotkey(Key::LeftBracket),
                        RewriteColor::ChangeAll(Color::ORANGE),
                        ctx,
                    )),
                    ManagedWidget::draw_text(ctx, Text::from(Line(actual_speed).size(14).roboto())),
                    ManagedWidget::btn(Button::rectangle_svg(
                        "assets/speed/speed_up.svg",
                        "speed up",
                        hotkey(Key::RightBracket),
                        RewriteColor::ChangeAll(Color::ORANGE),
                        ctx,
                    )),
                ]
                .into_iter()
                .map(|x| x.margin(5))
                .collect(),
            )
            .bg(Color::grey(0.5)),
        );

        Composite::new(ezgui::Composite::aligned_with_sliders(
            ctx,
            (
                HorizontalAlignment::Center,
                VerticalAlignment::BottomAboveOSD,
            ),
            ManagedWidget::row(row.into_iter().map(|x| x.margin(5)).collect())
                .bg(Color::hex("#4C4C4C")),
            vec![("speed", slider)],
        ))
        .cb(
            "jump to specific time",
            Box::new(|_, _| Some(Transition::Push(WizardState::new(Box::new(jump_to_time))))),
        )
        .cb(
            "step forwards 0.1 seconds",
            Box::new(|ctx, ui| {
                ui.primary.sim.step(&ui.primary.map, Duration::seconds(0.1));
                if let Some(ref mut s) = ui.secondary {
                    s.sim.step(&s.map, Duration::seconds(0.1));
                }
                ui.recalculate_current_selection(ctx);
                None
            }),
        )
        .cb(
            "step forwards 1 hour",
            Box::new(|_, ui| {
                Some(Transition::PushWithMode(
                    Box::new(TimeWarpScreen {
                        target: ui.primary.sim.time() + Duration::hours(1),
                        started: Instant::now(),
                    }),
                    EventLoopMode::Animation,
                ))
            }),
        )
    }

    pub fn new(ctx: &EventCtx, ui: &UI) -> SpeedControls {
        // 10 sim minutes / real second normally, or 1 sim hour / real second for dev mode
        let speed_cap: f64 = if ui.opts.dev { 3600.0 } else { 600.0 };
        let mut slider = Slider::horizontal(ctx, 160.0);
        // Start with speed=1.0
        slider.set_percent(ctx, (speed_cap / 1.0).powf(-1.0 / std::f64::consts::E));

        let now = Instant::now();
        let composite = SpeedControls::make_panel(ctx, false, "...", slider);

        SpeedControls {
            composite,

            state: SpeedState::Running {
                last_step: now,
                speed_description: "...".to_string(),
                last_measurement: now,
                last_measurement_sim: ui.primary.sim.time(),
            },
            speed_cap,
        }
    }

    pub fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Option<Outcome> {
        let desired_speed = self.desired_speed();
        match self.composite.event(ctx, ui) {
            Some(Outcome::Transition(t)) => {
                return Some(Outcome::Transition(t));
            }
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "speed up" => {
                    if desired_speed != self.speed_cap {
                        let percent = self.composite.slider("speed").get_percent();
                        self.composite
                            .mut_slider("speed")
                            .set_percent(ctx, (percent + ADJUST_SPEED_PERCENT).min(1.0));
                    }
                }
                "slow down" => {
                    if desired_speed != 0.0 {
                        let percent = self.composite.slider("speed").get_percent();
                        self.composite
                            .mut_slider("speed")
                            .set_percent(ctx, (percent - ADJUST_SPEED_PERCENT).max(0.0));
                    }
                }
                "resume" => {
                    let now = Instant::now();
                    self.state = SpeedState::Running {
                        last_step: now,
                        speed_description: "...".to_string(),
                        last_measurement: now,
                        last_measurement_sim: ui.primary.sim.time(),
                    };
                    self.composite = SpeedControls::make_panel(
                        ctx,
                        false,
                        "...",
                        self.composite.take_slider("speed"),
                    );
                    return None;
                }
                "pause" => {
                    self.pause(ctx);
                }
                "reset to midnight" => {
                    return Some(Outcome::Clicked("reset to midnight".to_string()));
                }
                _ => unreachable!(),
            },
            None => {}
        }

        if let SpeedState::Running {
            ref mut last_step,
            ref mut speed_description,
            ref mut last_measurement,
            ref mut last_measurement_sim,
        } = self.state
        {
            if ctx.input.nonblocking_is_update_event() {
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
                    self.composite = SpeedControls::make_panel(
                        ctx,
                        false,
                        &speed_description,
                        self.composite.take_slider("speed"),
                    );
                }
                // If speed is too high, don't be unresponsive for too long.
                // TODO This should probably match the ezgui framerate.
                ui.primary
                    .sim
                    .time_limited_step(&ui.primary.map, dt, Duration::seconds(0.1));
                ui.recalculate_current_selection(ctx);
            }
        }

        None
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        self.composite.draw(g);
    }

    pub fn pause(&mut self, ctx: &EventCtx) {
        if !self.is_paused() {
            self.state = SpeedState::Paused;
            self.composite =
                SpeedControls::make_panel(ctx, true, "...", self.composite.take_slider("speed"));
        }
    }

    pub fn is_paused(&self) -> bool {
        match self.state {
            SpeedState::Paused => true,
            SpeedState::Running { .. } => false,
        }
    }

    fn desired_speed(&self) -> f64 {
        self.speed_cap
            * self
                .composite
                .slider("speed")
                .get_percent()
                .powf(std::f64::consts::E)
    }
}

fn jump_to_time(wiz: &mut Wizard, ctx: &mut EventCtx, ui: &mut UI) -> Option<Transition> {
    let target = wiz.wrap(ctx).input_time_slider(
        "Jump to what time in the future?",
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

pub struct TimePanel {
    time: Time,
    composite: ezgui::Composite,
}

impl TimePanel {
    pub fn new(ctx: &EventCtx, ui: &UI) -> TimePanel {
        TimePanel {
            time: ui.primary.sim.time(),
            composite: ezgui::Composite::aligned(
                ctx,
                (HorizontalAlignment::Left, VerticalAlignment::Top),
                ManagedWidget::col(vec![
                    ManagedWidget::draw_text(
                        ctx,
                        Text::from(Line(ui.primary.sim.time().ampm_tostring()).size(30)),
                    )
                    .centered(),
                    {
                        let mut batch = GeomBatch::new();
                        // This is manually tuned
                        let width = 300.0;
                        let y1 = 5.0;
                        let height = Distance::meters(15.0);
                        let percent = ui.primary.sim.time().to_percent(Time::END_OF_DAY);

                        // TODO rounded
                        batch.push(
                            Color::WHITE,
                            Line::new(Pt2D::new(0.0, y1), Pt2D::new(width, y1))
                                .make_polygons(height),
                        );
                        if let Some(l) =
                            Line::maybe_new(Pt2D::new(0.0, y1), Pt2D::new(percent * width, y1))
                        {
                            batch.push(Color::grey(0.5), l.make_polygons(height));
                        }
                        ManagedWidget::draw_batch(ctx, batch).padding(5)
                    },
                    ManagedWidget::row(vec![
                        ManagedWidget::draw_text(ctx, Text::from(Line("00:00").size(12).roboto())),
                        ManagedWidget::draw_svg(ctx, "assets/speed/sunrise.svg"),
                        ManagedWidget::draw_text(ctx, Text::from(Line("12:00").size(12).roboto())),
                        ManagedWidget::draw_svg(ctx, "assets/speed/sunset.svg"),
                        ManagedWidget::draw_text(ctx, Text::from(Line("24:00").size(12).roboto())),
                    ])
                    .evenly_spaced(),
                ])
                .bg(Color::hex("#4C4C4C"))
                .padding(10),
            ),
        }
    }

    pub fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) {
        if self.time != ui.primary.sim.time() {
            *self = TimePanel::new(ctx, ui);
        }
        self.composite.event(ctx);
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        self.composite.draw(g);
    }
}
