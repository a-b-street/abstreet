use crate::colors;
use crate::game::{msg, State, Transition};
use crate::managed::{WrappedComposite, WrappedOutcome};
use crate::ui::UI;
use ezgui::{
    hotkey, Button, Color, Composite, EventCtx, EventLoopMode, GeomBatch, GfxCtx,
    HorizontalAlignment, Key, Line, ManagedWidget, Outcome, Plot, RewriteColor, Series, Slider,
    Text, VerticalAlignment,
};
use geom::{Duration, Polygon, Time};
use instant::Instant;

pub struct SpeedControls {
    pub composite: WrappedComposite,

    paused: bool,
    setting: SpeedSetting,
}

#[derive(Clone, Copy, PartialEq, PartialOrd)]
enum SpeedSetting {
    // 1 sim second per real second
    Realtime,
    // 5 sim seconds per real second
    Fast,
    // 30 sim seconds per real second
    Faster,
    // 1 sim hour per real second
    Fastest,
}

impl SpeedControls {
    fn make_panel(ctx: &mut EventCtx, paused: bool, setting: SpeedSetting) -> WrappedComposite {
        let mut row = Vec::new();
        row.push(
            ManagedWidget::btn(if paused {
                Button::rectangle_svg(
                    "../data/system/assets/speed/triangle.svg",
                    "play",
                    hotkey(Key::Space),
                    RewriteColor::ChangeAll(colors::HOVERING),
                    ctx,
                )
            } else {
                Button::rectangle_svg(
                    "../data/system/assets/speed/pause.svg",
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
                    (SpeedSetting::Realtime, "real-time speed"),
                    (SpeedSetting::Fast, "5x speed"),
                    (SpeedSetting::Faster, "30x speed"),
                    (SpeedSetting::Fastest, "3600x speed"),
                ]
                .into_iter()
                .map(|(s, label)| {
                    let mut tooltip = Text::from(Line(label).size(20)).with_bg();
                    tooltip.add(Line("[").fg(Color::GREEN).size(20));
                    tooltip.append(Line(" - slow down"));
                    tooltip.add(Line("]").fg(Color::GREEN).size(20));
                    tooltip.append(Line(" - speed up"));

                    ManagedWidget::btn(
                        Button::rectangle_svg_rewrite(
                            "../data/system/assets/speed/triangle.svg",
                            label,
                            None,
                            if setting >= s {
                                RewriteColor::NoOp
                            } else {
                                RewriteColor::ChangeAll(Color::WHITE.alpha(0.2))
                            },
                            RewriteColor::ChangeAll(colors::HOVERING),
                            ctx,
                        )
                        .change_tooltip(tooltip),
                    )
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
                        "../data/system/assets/speed/jump_to_time.svg",
                        "jump to specific time",
                        hotkey(Key::B),
                        RewriteColor::ChangeAll(colors::HOVERING),
                        ctx,
                    )),
                    ManagedWidget::btn(Button::rectangle_svg(
                        "../data/system/assets/speed/reset.svg",
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
            Box::new(|ctx, ui| Some(Transition::Push(Box::new(JumpToTime::new(ctx, ui))))),
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
            Box::new(|ctx, ui| {
                Some(Transition::Push(Box::new(TimeWarpScreen {
                    target: ui.primary.sim.time() + Duration::hours(1),
                    started: Instant::now(),
                    composite: Composite::new(ManagedWidget::draw_text(ctx, Text::new()))
                        .build(ctx),
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
                "real-time speed" => {
                    self.setting = SpeedSetting::Realtime;
                    self.composite = SpeedControls::make_panel(ctx, self.paused, self.setting);
                    return None;
                }
                "5x speed" => {
                    self.setting = SpeedSetting::Fast;
                    self.composite = SpeedControls::make_panel(ctx, self.paused, self.setting);
                    return None;
                }
                "30x speed" => {
                    self.setting = SpeedSetting::Faster;
                    self.composite = SpeedControls::make_panel(ctx, self.paused, self.setting);
                    return None;
                }
                "3600x speed" => {
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
                SpeedSetting::Fast => {
                    self.setting = SpeedSetting::Realtime;
                    self.composite = SpeedControls::make_panel(ctx, self.paused, self.setting);
                }
                SpeedSetting::Faster => {
                    self.setting = SpeedSetting::Fast;
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
                        self.setting = SpeedSetting::Fast;
                        self.composite = SpeedControls::make_panel(ctx, self.paused, self.setting);
                    }
                }
                SpeedSetting::Fast => {
                    self.setting = SpeedSetting::Faster;
                    self.composite = SpeedControls::make_panel(ctx, self.paused, self.setting);
                }
                SpeedSetting::Faster => {
                    self.setting = SpeedSetting::Fastest;
                    self.composite = SpeedControls::make_panel(ctx, self.paused, self.setting);
                }
                SpeedSetting::Fastest => {}
            }
        }

        if !self.paused {
            if let Some(real_dt) = ctx.input.nonblocking_is_update_event() {
                ctx.input.use_update_event();
                let multiplier = match self.setting {
                    SpeedSetting::Realtime => 1.0,
                    SpeedSetting::Fast => 5.0,
                    SpeedSetting::Faster => 30.0,
                    SpeedSetting::Fastest => 3600.0,
                };
                let dt = multiplier * real_dt;
                // TODO This should match the update frequency in ezgui. Plumb along the deadline
                // or frequency to here.
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

struct JumpToTime {
    composite: Composite,
    target: Time,
}

impl JumpToTime {
    fn new(ctx: &mut EventCtx, ui: &UI) -> JumpToTime {
        let target = ui.primary.sim.time();
        // TODO Auto-fill width?
        let mut slider = Slider::horizontal(ctx, 0.25 * ctx.canvas.window_width, 25.0);
        slider.set_percent(ctx, target.to_percent(Time::END_OF_DAY).min(1.0));
        JumpToTime {
            target,
            composite: Composite::new(
                ManagedWidget::col(vec![
                    WrappedComposite::text_button(ctx, "X", hotkey(Key::Escape)).align_right(),
                    ManagedWidget::draw_text(ctx, {
                        let mut txt = Text::from(Line("Jump to what time?").roboto_bold());
                        txt.add(Line(target.ampm_tostring()));
                        txt
                    })
                    .named("target time"),
                    ManagedWidget::slider("time slider").margin(10),
                    ManagedWidget::row(vec![
                        ManagedWidget::draw_text(ctx, Text::from(Line("00:00").size(12).roboto())),
                        ManagedWidget::draw_svg(ctx, "../data/system/assets/speed/sunrise.svg"),
                        ManagedWidget::draw_text(ctx, Text::from(Line("12:00").size(12).roboto())),
                        ManagedWidget::draw_svg(ctx, "../data/system/assets/speed/sunset.svg"),
                        ManagedWidget::draw_text(ctx, Text::from(Line("24:00").size(12).roboto())),
                    ])
                    .padding(10)
                    .evenly_spaced(),
                    WrappedComposite::text_bg_button(ctx, "Go!", hotkey(Key::Enter))
                        .centered_horiz(),
                    ManagedWidget::draw_text(ctx, Text::from(Line("Active agents").roboto_bold())),
                    Plot::new_usize(
                        vec![Series {
                            label: (if ui.has_prebaked().is_some() {
                                "Baseline"
                            } else {
                                "Current simulation"
                            })
                            .to_string(),
                            color: Color::RED,
                            pts: (if ui.has_prebaked().is_some() {
                                ui.prebaked()
                            } else {
                                ui.primary.sim.get_analytics()
                            })
                            .active_agents(Time::END_OF_DAY),
                        }],
                        ctx,
                    ),
                ])
                .bg(colors::PANEL_BG),
            )
            .slider("time slider", slider)
            .build(ctx),
        }
    }
}

impl State for JumpToTime {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "X" => {
                    return Transition::Pop;
                }
                "Go!" => {
                    if self.target < ui.primary.sim.time() {
                        // TODO Make it possible!
                        return Transition::Replace(msg(
                            "Error",
                            vec![
                                "You can't use this to rewind time yet.".to_string(),
                                "Click the reset to midnight button first.".to_string(),
                            ],
                        ));
                    }
                    return Transition::Replace(Box::new(TimeWarpScreen {
                        target: self.target,
                        started: Instant::now(),
                        composite: Composite::new(ManagedWidget::draw_text(ctx, Text::new()))
                            .build(ctx),
                    }));
                }
                _ => unreachable!(),
            },
            None => {}
        }
        let target =
            Time::END_OF_DAY.percent_of(self.composite.slider("time slider").get_percent());
        if target != self.target {
            self.target = target;
            self.composite.replace(
                ctx,
                "target time",
                ManagedWidget::draw_text(ctx, {
                    let mut txt = Text::from(Line("Jump to what time?").roboto_bold());
                    txt.add(Line(target.ampm_tostring()));
                    txt
                })
                .named("target time"),
            );
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &UI) {
        State::grey_out_map(g);
        self.composite.draw(g);
    }
}

// Display a nicer screen for jumping forwards in time, allowing cancellation.
pub struct TimeWarpScreen {
    target: Time,
    started: Instant,
    composite: Composite,
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

            // TODO Instead display base speed controls, some indication of target time and ability
            // to cancel
            let mut txt = Text::from(Line("Warping through time...").roboto_bold());
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
            txt.add(Line(""));
            txt.add(Line(format!("Press ESCAPE to stop now")));

            self.composite = Composite::new(
                ManagedWidget::draw_text(ctx, txt)
                    .padding(10)
                    .bg(colors::PANEL_BG)
                    .outline(5.0, Color::WHITE),
            )
            .build(ctx);
        }
        if ui.primary.sim.time() == self.target {
            return Transition::Pop;
        }

        Transition::KeepWithMode(EventLoopMode::Animation)
    }

    fn draw(&self, g: &mut GfxCtx, _: &UI) {
        State::grey_out_map(g);
        self.composite.draw(g);
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
                    .margin(10)
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
                        ManagedWidget::draw_svg(ctx, "../data/system/assets/speed/sunrise.svg"),
                        ManagedWidget::draw_text(ctx, Text::from(Line("12:00").size(12).roboto())),
                        ManagedWidget::draw_svg(ctx, "../data/system/assets/speed/sunset.svg"),
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
