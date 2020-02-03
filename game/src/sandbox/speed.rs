use crate::colors;
use crate::game::{State, Transition, WizardState};
use crate::managed::{WrappedComposite, WrappedOutcome};
use crate::ui::UI;
use ezgui::{
    hotkey, Button, Color, Composite, EventCtx, EventLoopMode, GeomBatch, GfxCtx,
    HorizontalAlignment, Key, Line, ManagedWidget, RewriteColor, Text, VerticalAlignment, Wizard,
};
use geom::{Duration, Polygon, Time};
use std::time::Instant;

pub struct SpeedControls {
    pub composite: WrappedComposite,

    paused: bool,
    setting: SpeedSetting,
}

#[derive(Clone, Copy, PartialEq, PartialOrd)]
enum SpeedSetting {
    // 1 sim second per real second
    Realtime,
    // 1 sim minute per real second
    MinutePerSec,
    // 1 sim hour per real second
    HourPerSec,
    // as fast as possible
    Uncapped,
}

impl SpeedControls {
    fn make_panel(ctx: &mut EventCtx, paused: bool, setting: SpeedSetting) -> WrappedComposite {
        let mut row = Vec::new();
        row.push(
            ManagedWidget::btn(if paused {
                Button::rectangle_svg(
                    "assets/speed/triangle.svg",
                    "play",
                    hotkey(Key::Space),
                    RewriteColor::ChangeAll(colors::HOVERING),
                    ctx,
                )
            } else {
                Button::rectangle_svg(
                    "assets/speed/pause.svg",
                    "pause",
                    hotkey(Key::Space),
                    RewriteColor::ChangeAll(colors::HOVERING),
                    ctx,
                )
            })
            .margin(5)
            .centered_vert()
            .bg(colors::SECTION_BG),
        );

        row.push(
            ManagedWidget::row(
                vec![
                    (SpeedSetting::Realtime, "realtime"),
                    (SpeedSetting::MinutePerSec, "60x"),
                    (SpeedSetting::HourPerSec, "3600x"),
                    (SpeedSetting::Uncapped, "as fast as possible"),
                ]
                .into_iter()
                .map(|(s, label)| {
                    ManagedWidget::btn(Button::rectangle_svg_rewrite(
                        "assets/speed/triangle.svg",
                        label,
                        None,
                        if setting >= s {
                            RewriteColor::NoOp
                        } else {
                            RewriteColor::ChangeAll(Color::WHITE.alpha(0.2))
                        },
                        RewriteColor::ChangeAll(colors::HOVERING),
                        ctx,
                    ))
                    .margin(5)
                })
                .collect(),
            )
            .bg(colors::SECTION_BG)
            .centered(),
        );

        row.push(
            ManagedWidget::row(
                vec![
                    ManagedWidget::btn(Button::text_no_bg(
                        Text::from(Line("+0.1s").fg(Color::WHITE).size(21).roboto()),
                        Text::from(Line("+0.1s").fg(colors::HOVERING).size(21).roboto()),
                        hotkey(Key::M),
                        "step forwards 0.1 seconds",
                        false,
                        ctx,
                    )),
                    ManagedWidget::btn(Button::text_no_bg(
                        Text::from(Line("+1h").fg(Color::WHITE).size(21).roboto()),
                        Text::from(Line("+1h").fg(colors::HOVERING).size(21).roboto()),
                        hotkey(Key::N),
                        "step forwards 1 hour",
                        false,
                        ctx,
                    )),
                    ManagedWidget::btn(Button::rectangle_svg(
                        "assets/speed/jump_to_time.svg",
                        "jump to specific time",
                        hotkey(Key::B),
                        RewriteColor::ChangeAll(colors::HOVERING),
                        ctx,
                    )),
                    ManagedWidget::btn(Button::rectangle_svg(
                        "assets/speed/reset.svg",
                        "reset to midnight",
                        hotkey(Key::X),
                        RewriteColor::ChangeAll(colors::HOVERING),
                        ctx,
                    )),
                ]
                .into_iter()
                .map(|x| x.margin(5))
                .collect(),
            )
            .bg(colors::SECTION_BG)
            .centered(),
        );

        WrappedComposite::new(
            Composite::new(
                ManagedWidget::row(row.into_iter().map(|x| x.margin(5)).collect())
                    .bg(colors::PANEL_BG),
            )
            .aligned(
                HorizontalAlignment::Center,
                VerticalAlignment::BottomAboveOSD,
            )
            .build(ctx),
        )
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
                Some(Transition::Push(Box::new(TimeWarpScreen {
                    target: ui.primary.sim.time() + Duration::hours(1),
                    started: Instant::now(),
                })))
            }),
        )
    }

    pub fn new(ctx: &mut EventCtx) -> SpeedControls {
        let composite = SpeedControls::make_panel(ctx, false, SpeedSetting::Realtime);
        SpeedControls {
            composite,
            paused: false,
            setting: SpeedSetting::Realtime,
        }
    }

    pub fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Option<WrappedOutcome> {
        match self.composite.event(ctx, ui) {
            Some(WrappedOutcome::Transition(t)) => {
                return Some(WrappedOutcome::Transition(t));
            }
            Some(WrappedOutcome::Clicked(x)) => match x.as_ref() {
                "realtime" => {
                    self.setting = SpeedSetting::Realtime;
                    self.composite = SpeedControls::make_panel(ctx, self.paused, self.setting);
                    return None;
                }
                "60x" => {
                    self.setting = SpeedSetting::MinutePerSec;
                    self.composite = SpeedControls::make_panel(ctx, self.paused, self.setting);
                    return None;
                }
                "3600x" => {
                    self.setting = SpeedSetting::HourPerSec;
                    self.composite = SpeedControls::make_panel(ctx, self.paused, self.setting);
                    return None;
                }
                "as fast as possible" => {
                    self.setting = SpeedSetting::Uncapped;
                    self.composite = SpeedControls::make_panel(ctx, self.paused, self.setting);
                    return None;
                }
                "play" => {
                    self.paused = false;
                    self.composite = SpeedControls::make_panel(ctx, self.paused, self.setting);
                    return None;
                }
                "pause" => {
                    self.pause(ctx);
                }
                "reset to midnight" => {
                    return Some(WrappedOutcome::Clicked("reset to midnight".to_string()));
                }
                _ => unreachable!(),
            },
            None => {}
        }

        // TODO How to communicate these keys?
        if ctx.input.new_was_pressed(hotkey(Key::LeftBracket).unwrap()) {
            match self.setting {
                SpeedSetting::Realtime => self.pause(ctx),
                SpeedSetting::MinutePerSec => {
                    self.setting = SpeedSetting::Realtime;
                    self.composite = SpeedControls::make_panel(ctx, self.paused, self.setting);
                }
                SpeedSetting::HourPerSec => {
                    self.setting = SpeedSetting::MinutePerSec;
                    self.composite = SpeedControls::make_panel(ctx, self.paused, self.setting);
                }
                SpeedSetting::Uncapped => {
                    self.setting = SpeedSetting::HourPerSec;
                    self.composite = SpeedControls::make_panel(ctx, self.paused, self.setting);
                }
            }
        }
        if ctx
            .input
            .new_was_pressed(hotkey(Key::RightBracket).unwrap())
        {
            match self.setting {
                SpeedSetting::Realtime => {
                    if self.paused {
                        self.paused = false;
                        self.composite = SpeedControls::make_panel(ctx, self.paused, self.setting);
                    } else {
                        self.setting = SpeedSetting::MinutePerSec;
                        self.composite = SpeedControls::make_panel(ctx, self.paused, self.setting);
                    }
                }
                SpeedSetting::MinutePerSec => {
                    self.setting = SpeedSetting::HourPerSec;
                    self.composite = SpeedControls::make_panel(ctx, self.paused, self.setting);
                }
                SpeedSetting::HourPerSec => {
                    self.setting = SpeedSetting::Uncapped;
                    self.composite = SpeedControls::make_panel(ctx, self.paused, self.setting);
                }
                SpeedSetting::Uncapped => {}
            }
        }

        if !self.paused {
            if let Some(real_dt) = ctx.input.nonblocking_is_update_event() {
                ctx.input.use_update_event();
                let multiplier = match self.setting {
                    SpeedSetting::Realtime => 1.0,
                    SpeedSetting::MinutePerSec => 60.0,
                    SpeedSetting::HourPerSec => 3600.0,
                    SpeedSetting::Uncapped => 10.0e9,
                };
                let dt = multiplier * real_dt;
                ui.primary
                    .sim
                    .time_limited_step(&ui.primary.map, dt, Duration::seconds(0.033));
                ui.recalculate_current_selection(ctx);
            }
        }

        None
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        self.composite.draw(g);
    }

    pub fn pause(&mut self, ctx: &mut EventCtx) {
        if !self.paused {
            self.paused = true;
            self.composite = SpeedControls::make_panel(ctx, self.paused, self.setting);
        }
    }

    pub fn resume_realtime(&mut self, ctx: &mut EventCtx) {
        if self.paused || self.setting != SpeedSetting::Realtime {
            self.paused = false;
            self.setting = SpeedSetting::Realtime;
            self.composite = SpeedControls::make_panel(ctx, self.paused, self.setting);
        }
    }

    pub fn is_paused(&self) -> bool {
        self.paused
    }
}

fn jump_to_time(wiz: &mut Wizard, ctx: &mut EventCtx, ui: &mut UI) -> Option<Transition> {
    let target = wiz.wrap(ctx).input_time_slider(
        "Jump to what time in the future?",
        ui.primary.sim.time(),
        Time::END_OF_DAY,
    )?;
    Some(Transition::Replace(Box::new(TimeWarpScreen {
        target,
        started: Instant::now(),
    })))
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
        if ctx.input.nonblocking_is_update_event().is_some() {
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
    pub composite: Composite,
}

impl TimePanel {
    pub fn new(ctx: &mut EventCtx, ui: &UI) -> TimePanel {
        TimePanel {
            time: ui.primary.sim.time(),
            composite: Composite::new(
                ManagedWidget::col(vec![
                    ManagedWidget::draw_text(
                        ctx,
                        Text::from(Line(ui.primary.sim.time().ampm_tostring()).size(30)),
                    )
                    .padding(10)
                    .centered_horiz(),
                    {
                        let mut batch = GeomBatch::new();
                        // This is manually tuned
                        let width = 300.0;
                        let height = 15.0;
                        // Just clamp past 24 hours
                        let percent = ui.primary.sim.time().to_percent(Time::END_OF_DAY).min(1.0);

                        // TODO rounded
                        batch.push(Color::WHITE, Polygon::rectangle(width, height));
                        if percent != 0.0 {
                            batch.push(
                                colors::SECTION_BG,
                                Polygon::rectangle(percent * width, height),
                            );
                        }
                        ManagedWidget::draw_batch(ctx, batch)
                    },
                    ManagedWidget::row(vec![
                        ManagedWidget::draw_text(ctx, Text::from(Line("00:00").size(12).roboto())),
                        ManagedWidget::draw_svg(ctx, "assets/speed/sunrise.svg"),
                        ManagedWidget::draw_text(ctx, Text::from(Line("12:00").size(12).roboto())),
                        ManagedWidget::draw_svg(ctx, "assets/speed/sunset.svg"),
                        ManagedWidget::draw_text(ctx, Text::from(Line("24:00").size(12).roboto())),
                    ])
                    .padding(10)
                    .evenly_spaced(),
                ])
                .padding(10)
                .bg(colors::PANEL_BG),
            )
            .aligned(HorizontalAlignment::Left, VerticalAlignment::Top)
            .build(ctx),
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
