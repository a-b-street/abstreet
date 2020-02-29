use crate::colors;
use crate::common::{Overlays, Warping};
use crate::game::{msg, State, Transition};
use crate::helpers::ID;
use crate::managed::{WrappedComposite, WrappedOutcome};
use crate::sandbox::{GameplayMode, SandboxMode};
use crate::ui::UI;
use ezgui::{
    hotkey, Button, Color, Composite, EventCtx, EventLoopMode, GeomBatch, GfxCtx,
    HorizontalAlignment, Key, Line, ManagedWidget, Outcome, Plot, PlotOptions, RewriteColor,
    Series, Slider, Text, VerticalAlignment,
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
                    let mut tooltip = Text::from(Line(label).size(20));
                    tooltip.add(Line(Key::LeftArrow.describe()).fg(Color::GREEN).size(20));
                    tooltip.append(Line(" - slow down"));
                    tooltip.add(Line(Key::RightArrow.describe()).fg(Color::GREEN).size(20));
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
                        Text::from(Line("+1h").fg(Color::WHITE).size(21).roboto()),
                        Text::from(Line("+1h").fg(colors::HOVERING).size(21).roboto()),
                        hotkey(Key::N),
                        "step forwards 1 hour",
                        false,
                        ctx,
                    )),
                    ManagedWidget::btn(Button::text_no_bg(
                        Text::from(Line("+0.1s").fg(Color::WHITE).size(21).roboto()),
                        Text::from(Line("+0.1s").fg(colors::HOVERING).size(21).roboto()),
                        hotkey(Key::M),
                        "step forwards 0.1 seconds",
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
            "step forwards 0.1 seconds",
            Box::new(|ctx, ui| {
                ui.primary
                    .sim
                    .normal_step(&ui.primary.map, Duration::seconds(0.1));
                if let Some(ref mut s) = ui.secondary {
                    s.sim.normal_step(&s.map, Duration::seconds(0.1));
                }
                ui.recalculate_current_selection(ctx);
                None
            }),
        )
        .cb(
            "step forwards 1 hour",
            Box::new(|ctx, ui| {
                Some(Transition::Push(Box::new(TimeWarpScreen::new(
                    ctx,
                    ui,
                    ui.primary.sim.time() + Duration::hours(1),
                    false,
                ))))
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

    pub fn event(
        &mut self,
        ctx: &mut EventCtx,
        ui: &mut UI,
        maybe_mode: Option<&GameplayMode>,
    ) -> Option<Transition> {
        match self.composite.event(ctx, ui) {
            Some(WrappedOutcome::Transition(t)) => {
                return Some(t);
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
                    if let Some(mode) = maybe_mode {
                        ui.primary.clear_sim();
                        return Some(Transition::Replace(Box::new(SandboxMode::new(
                            ctx,
                            ui,
                            mode.clone(),
                        ))));
                    } else {
                        return Some(Transition::Push(msg(
                            "Error",
                            vec!["Sorry, you can't go rewind time from this mode."],
                        )));
                    }
                }
                "jump to specific time" => {
                    return Some(Transition::Push(Box::new(JumpToTime::new(
                        ctx,
                        ui,
                        maybe_mode.cloned(),
                    ))));
                }
                _ => unreachable!(),
            },
            None => {}
        }

        if ctx.input.new_was_pressed(&hotkey(Key::LeftArrow).unwrap()) {
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
        if ctx.input.new_was_pressed(&hotkey(Key::RightArrow).unwrap()) {
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

// TODO Text entry would be great
struct JumpToTime {
    composite: Composite,
    target: Time,
    maybe_mode: Option<GameplayMode>,
    traffic_jams: bool,
}

impl JumpToTime {
    fn new(ctx: &mut EventCtx, ui: &UI, maybe_mode: Option<GameplayMode>) -> JumpToTime {
        let target = ui.primary.sim.time();
        // TODO Auto-fill width?
        let mut slider = Slider::horizontal(ctx, 0.25 * ctx.canvas.window_width, 25.0);
        slider.set_percent(ctx, target.to_percent(Time::END_OF_DAY).min(1.0));
        JumpToTime {
            target,
            maybe_mode,
            traffic_jams: false,
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
                    WrappedComposite::text_button(ctx, "☐ Stop when there's a traffic jam", None)
                        .named("traffic jams")
                        .padding(10)
                        .margin(10),
                    WrappedComposite::text_bg_button(ctx, "Go!", hotkey(Key::Enter))
                        .centered_horiz(),
                    ManagedWidget::draw_text(ctx, Text::from(Line("Active agents").roboto_bold())),
                    // TODO Sync the slider / plot.
                    Plot::new_usize(
                        ctx,
                        vec![if ui.has_prebaked().is_some() {
                            Series {
                                label: "Baseline".to_string(),
                                color: Color::BLUE,
                                pts: ui.prebaked().active_agents(Time::END_OF_DAY),
                            }
                        } else {
                            Series {
                                label: "Current simulation".to_string(),
                                color: Color::RED,
                                pts: ui
                                    .primary
                                    .sim
                                    .get_analytics()
                                    .active_agents(ui.primary.sim.time()),
                            }
                        }],
                        PlotOptions {
                            max_x: Some(Time::END_OF_DAY),
                        },
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
                "☐ Stop when there's a traffic jam" => {
                    self.traffic_jams = true;
                    self.composite.replace(
                        ctx,
                        "traffic jams",
                        WrappedComposite::text_button(
                            ctx,
                            "☑ Stop when there's a traffic jam",
                            None,
                        )
                        .named("traffic jams")
                        .padding(10)
                        .margin(10),
                    );
                }
                "☑ Stop when there's a traffic jam" => {
                    self.traffic_jams = false;
                    self.composite.replace(
                        ctx,
                        "traffic jams",
                        WrappedComposite::text_button(
                            ctx,
                            "☐ Stop when there's a traffic jam",
                            None,
                        )
                        .named("traffic jams")
                        .padding(10)
                        .margin(10),
                    );
                }
                "Go!" => {
                    if self.target < ui.primary.sim.time() {
                        if let Some(mode) = self.maybe_mode.take() {
                            ui.primary.clear_sim();
                            return Transition::ReplaceThenPush(
                                Box::new(SandboxMode::new(ctx, ui, mode)),
                                Box::new(TimeWarpScreen::new(
                                    ctx,
                                    ui,
                                    self.target,
                                    self.traffic_jams,
                                )),
                            );
                        } else {
                            return Transition::Replace(msg(
                                "Error",
                                vec!["Sorry, you can't go rewind time from this mode."],
                            ));
                        }
                    }
                    return Transition::Replace(Box::new(TimeWarpScreen::new(
                        ctx,
                        ui,
                        self.target,
                        self.traffic_jams,
                    )));
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
                    // TODO The panel jumps too much and the slider position changes place.
                    /*if target < ui.primary.sim.time() {
                        txt.add(Line("(Going back in time will reset to midnight, then simulate forwards)"));
                    }*/
                    txt
                })
                .named("target time"),
            );
        }
        if self.composite.clicked_outside(ctx) {
            return Transition::Pop;
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
    traffic_jams: bool,
    composite: Composite,
}

impl TimeWarpScreen {
    fn new(ctx: &mut EventCtx, ui: &mut UI, target: Time, traffic_jams: bool) -> TimeWarpScreen {
        if traffic_jams {
            ui.primary
                .sim
                .set_gridlock_checker(Some(Duration::minutes(5)));
        }

        TimeWarpScreen {
            target,
            started: Instant::now(),
            traffic_jams,
            composite: Composite::new(
                ManagedWidget::col(vec![
                    ManagedWidget::draw_text(ctx, Text::new()).named("text"),
                    WrappedComposite::text_bg_button(ctx, "stop now", hotkey(Key::Escape))
                        .centered_horiz(),
                ])
                .padding(10)
                .bg(colors::PANEL_BG),
            )
            .build(ctx),
        }
    }
}

impl State for TimeWarpScreen {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        if ctx.input.nonblocking_is_update_event().is_some() {
            ctx.input.use_update_event();
            if let Some(problems) = ui.primary.sim.time_limited_step(
                &ui.primary.map,
                self.target - ui.primary.sim.time(),
                Duration::seconds(0.033),
            ) {
                let id = ID::Intersection(problems[0].0);
                ui.overlay = Overlays::traffic_jams(ctx, ui);
                return Transition::Replace(Warping::new(
                    ctx,
                    id.canonical_point(&ui.primary).unwrap(),
                    Some(10.0),
                    Some(id),
                    &mut ui.primary,
                ));
            }
            // TODO secondary for a/b test mode

            // I'm covered in shame for not doing this from the start.
            let mut txt = Text::from(Line("Let's do the time warp again!").roboto_bold());
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

            self.composite.replace(
                ctx,
                "text",
                ManagedWidget::draw_text(ctx, txt).named("text"),
            );
        }
        if ui.primary.sim.time() == self.target {
            return Transition::Pop;
        }

        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "stop now" => {
                    return Transition::Pop;
                }
                _ => unreachable!(),
            },
            None => {}
        }
        if self.composite.clicked_outside(ctx) {
            return Transition::Pop;
        }

        Transition::KeepWithMode(EventLoopMode::Animation)
    }

    fn draw(&self, g: &mut GfxCtx, _: &UI) {
        State::grey_out_map(g);
        self.composite.draw(g);
    }

    fn on_destroy(&mut self, _: &mut EventCtx, ui: &mut UI) {
        if self.traffic_jams {
            ui.primary.sim.set_gridlock_checker(None);
        }
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
