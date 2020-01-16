use crate::game::{State, Transition, WizardState};
use crate::managed::{Composite, Outcome};
use crate::ui::UI;
use ezgui::{
    hotkey, Button, Color, EventCtx, EventLoopMode, GeomBatch, GfxCtx, HorizontalAlignment, Key,
    Line, ManagedWidget, RewriteColor, Text, VerticalAlignment, Wizard,
};
use geom::{Distance, Duration, Line, Pt2D, Time};
use std::time::Instant;

pub struct SpeedControls {
    composite: Composite,

    paused: bool,
    setting: SpeedSetting,
}

#[derive(Clone, Copy)]
enum SpeedSetting {
    Realtime,
    Faster,
    Fastest,
}

impl SpeedControls {
    fn make_panel(ctx: &mut EventCtx, paused: bool, setting: SpeedSetting) -> Composite {
        let bg = Color::hex("#7C7C7C");

        let mut row = Vec::new();
        if paused {
            row.push(
                ManagedWidget::row(vec![ManagedWidget::btn(Button::rectangle_svg(
                    "assets/speed/triangle.svg",
                    "play",
                    hotkey(Key::Space),
                    RewriteColor::ChangeAll(Color::ORANGE),
                    ctx,
                ))])
                .bg(bg)
                .margin(5),
            );
        } else {
            row.push(
                ManagedWidget::row(vec![ManagedWidget::btn(Button::rectangle_svg(
                    "assets/speed/pause.svg",
                    "pause",
                    hotkey(Key::Space),
                    RewriteColor::ChangeAll(Color::ORANGE),
                    ctx,
                ))])
                .bg(bg)
                .margin(5),
            );
        }

        let mut settings = vec![ManagedWidget::btn(Button::rectangle_svg(
            "assets/speed/triangle.svg",
            "realtime",
            None,
            RewriteColor::ChangeAll(Color::ORANGE),
            ctx,
        ))];
        match setting {
            SpeedSetting::Realtime => {
                settings.push(ManagedWidget::btn(Button::rectangle_svg_rewrite(
                    "assets/speed/triangle.svg",
                    "600x speed",
                    None,
                    RewriteColor::ChangeAll(Color::WHITE.alpha(0.2)),
                    RewriteColor::ChangeAll(Color::ORANGE),
                    ctx,
                )));
                settings.push(ManagedWidget::btn(Button::rectangle_svg_rewrite(
                    "assets/speed/triangle.svg",
                    "as fast as possible",
                    None,
                    RewriteColor::ChangeAll(Color::WHITE.alpha(0.2)),
                    RewriteColor::ChangeAll(Color::ORANGE),
                    ctx,
                )));
            }
            SpeedSetting::Faster => {
                settings.push(ManagedWidget::btn(Button::rectangle_svg(
                    "assets/speed/triangle.svg",
                    "600x speed",
                    None,
                    RewriteColor::ChangeAll(Color::ORANGE),
                    ctx,
                )));
                settings.push(ManagedWidget::btn(Button::rectangle_svg_rewrite(
                    "assets/speed/triangle.svg",
                    "as fast as possible",
                    None,
                    RewriteColor::ChangeAll(Color::WHITE.alpha(0.2)),
                    RewriteColor::ChangeAll(Color::ORANGE),
                    ctx,
                )));
            }
            SpeedSetting::Fastest => {
                settings.push(ManagedWidget::btn(Button::rectangle_svg(
                    "assets/speed/triangle.svg",
                    "600x speed",
                    None,
                    RewriteColor::ChangeAll(Color::ORANGE),
                    ctx,
                )));
                settings.push(ManagedWidget::btn(Button::rectangle_svg(
                    "assets/speed/triangle.svg",
                    "as fast as possible",
                    None,
                    RewriteColor::ChangeAll(Color::ORANGE),
                    ctx,
                )));
            }
        }
        row.push(ManagedWidget::row(settings.into_iter().map(|x| x.margin(5)).collect()).bg(bg));

        row.push(
            ManagedWidget::row(
                vec![
                    ManagedWidget::btn(Button::text_no_bg(
                        Text::from(Line("+0.1s").fg(Color::WHITE).size(21).roboto()),
                        Text::from(Line("+0.1s").fg(Color::ORANGE).size(21).roboto()),
                        hotkey(Key::M),
                        "step forwards 0.1 seconds",
                        ctx,
                    )),
                    ManagedWidget::btn(Button::text_no_bg(
                        Text::from(Line("+1h").fg(Color::WHITE).size(21).roboto()),
                        Text::from(Line("+1h").fg(Color::ORANGE).size(21).roboto()),
                        hotkey(Key::N),
                        "step forwards 1 hour",
                        ctx,
                    )),
                    ManagedWidget::btn(Button::rectangle_svg(
                        "assets/speed/jump_to_time.svg",
                        "jump to specific time",
                        hotkey(Key::B),
                        RewriteColor::ChangeAll(Color::ORANGE),
                        ctx,
                    )),
                    ManagedWidget::btn(Button::rectangle_svg(
                        "assets/speed/reset.svg",
                        "reset to midnight",
                        hotkey(Key::X),
                        RewriteColor::ChangeAll(Color::ORANGE),
                        ctx,
                    )),
                ]
                .into_iter()
                .map(|x| x.margin(5))
                .collect(),
            )
            .bg(bg),
        );

        Composite::new(
            ezgui::Composite::new(
                ManagedWidget::row(row.into_iter().map(|x| x.margin(5)).collect())
                    .bg(Color::hex("#4C4C4C")),
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

    pub fn new(ctx: &mut EventCtx) -> SpeedControls {
        let composite = SpeedControls::make_panel(ctx, false, SpeedSetting::Realtime);
        SpeedControls {
            composite,
            paused: false,
            setting: SpeedSetting::Realtime,
        }
    }

    pub fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Option<Outcome> {
        match self.composite.event(ctx, ui) {
            Some(Outcome::Transition(t)) => {
                return Some(Outcome::Transition(t));
            }
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "realtime" => {
                    self.setting = SpeedSetting::Realtime;
                    self.composite = SpeedControls::make_panel(ctx, self.paused, self.setting);
                    return None;
                }
                "600x speed" => {
                    self.setting = SpeedSetting::Faster;
                    self.composite = SpeedControls::make_panel(ctx, self.paused, self.setting);
                    return None;
                }
                "as fast as possible" => {
                    self.setting = SpeedSetting::Fastest;
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
                    return Some(Outcome::Clicked("reset to midnight".to_string()));
                }
                _ => unreachable!(),
            },
            None => {}
        }

        // TODO How to communicate these keys?
        if ctx.input.new_was_pressed(hotkey(Key::LeftBracket).unwrap()) {
            match self.setting {
                SpeedSetting::Realtime => self.pause(ctx),
                SpeedSetting::Faster => {
                    self.setting = SpeedSetting::Realtime;
                    self.composite = SpeedControls::make_panel(ctx, self.paused, self.setting);
                }
                SpeedSetting::Fastest => {
                    self.setting = SpeedSetting::Faster;
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
                        self.setting = SpeedSetting::Faster;
                        self.composite = SpeedControls::make_panel(ctx, self.paused, self.setting);
                    }
                }
                SpeedSetting::Faster => {
                    self.setting = SpeedSetting::Fastest;
                    self.composite = SpeedControls::make_panel(ctx, self.paused, self.setting);
                }
                SpeedSetting::Fastest => {}
            }
        }

        if !self.paused && ctx.input.nonblocking_is_update_event() {
            // TODO This is very wrong. Actually cap realtime and faster to 1x and something else
            // (10 minutes/s?), factoring in ezgui framerate.
            ctx.input.use_update_event();
            let max_step = match self.setting {
                SpeedSetting::Realtime => Duration::seconds(0.1),
                SpeedSetting::Faster => Duration::minutes(1),
                SpeedSetting::Fastest => Duration::hours(24),
            };
            ui.primary
                .sim
                .time_limited_step(&ui.primary.map, max_step, Duration::seconds(0.1));
            ui.recalculate_current_selection(ctx);
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
    pub fn new(ctx: &mut EventCtx, ui: &UI) -> TimePanel {
        TimePanel {
            time: ui.primary.sim.time(),
            composite: ezgui::Composite::new(
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
                        // Just clamp past 24 hours
                        let percent = ui.primary.sim.time().to_percent(Time::END_OF_DAY).min(1.0);

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
