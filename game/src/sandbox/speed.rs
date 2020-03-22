use crate::app::App;
use crate::colors;
use crate::common::{Overlays, Warping};
use crate::game::{msg, State, Transition};
use crate::helpers::ID;
use crate::managed::{WrappedComposite, WrappedOutcome};
use crate::sandbox::{GameplayMode, SandboxMode};
use ezgui::{
    hotkey, Btn, Color, Composite, EventCtx, EventLoopMode, GeomBatch, GfxCtx, HorizontalAlignment,
    Key, Line, Outcome, Plot, PlotOptions, RewriteColor, Series, Slider, Text, VerticalAlignment,
    Widget,
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
    // TODO Could use custom_checkbox here, but not sure it'll make things that much simpler.
    fn make_panel(ctx: &mut EventCtx, paused: bool, setting: SpeedSetting) -> WrappedComposite {
        let mut row = Vec::new();
        row.push(
            if paused {
                Btn::svg_def("../data/system/assets/speed/triangle.svg").build(
                    ctx,
                    "play",
                    hotkey(Key::Space),
                )
            } else {
                Btn::svg_def("../data/system/assets/speed/pause.svg").build(
                    ctx,
                    "pause",
                    hotkey(Key::Space),
                )
            }
            .margin(5)
            .centered_vert()
            .bg(colors::SECTION_BG),
        );

        row.push(
            Widget::row(
                vec![
                    (SpeedSetting::Realtime, "real-time speed"),
                    (SpeedSetting::Fast, "5x speed"),
                    (SpeedSetting::Faster, "30x speed"),
                    (SpeedSetting::Fastest, "3600x speed"),
                ]
                .into_iter()
                .map(|(s, label)| {
                    let mut txt = Text::from(Line(label).size(20));
                    txt.add(Line(Key::LeftArrow.describe()).fg(Color::GREEN).size(20));
                    txt.append(Line(" - slow down"));
                    txt.add(Line(Key::RightArrow.describe()).fg(Color::GREEN).size(20));
                    txt.append(Line(" - speed up"));

                    let (normal, bounds) = GeomBatch::from_svg(
                        ctx,
                        "../data/system/assets/speed/triangle.svg",
                        if setting >= s {
                            RewriteColor::NoOp
                        } else {
                            RewriteColor::ChangeAll(Color::WHITE.alpha(0.2))
                        },
                    );
                    let mut hovered = normal.clone();
                    hovered.rewrite_color(RewriteColor::ChangeAll(colors::HOVERING));

                    Btn::custom(normal, hovered, bounds.get_rectangle())
                        .tooltip(txt)
                        .build(ctx, label, None)
                        .margin(5)
                })
                .collect(),
            )
            .bg(colors::SECTION_BG)
            .centered(),
        );

        row.push(
            Widget::row(
                vec![
                    Btn::custom(
                        Text::from(Line("+1h").fg(Color::WHITE).size(21).roboto()).render_ctx(ctx),
                        Text::from(Line("+1h").fg(colors::HOVERING).size(21).roboto())
                            .render_ctx(ctx),
                        {
                            let dims = Text::from(Line("+1h").size(21).roboto())
                                .render_ctx(ctx)
                                .get_dims();
                            Polygon::rectangle(dims.width, dims.height)
                        },
                    )
                    .build(ctx, "step forwards 1 hour", hotkey(Key::N)),
                    Btn::custom(
                        Text::from(Line("+0.1s").fg(Color::WHITE).size(21).roboto())
                            .render_ctx(ctx),
                        Text::from(Line("+0.1s").fg(colors::HOVERING).size(21).roboto())
                            .render_ctx(ctx),
                        {
                            let dims = Text::from(Line("+0.1s").size(21).roboto())
                                .render_ctx(ctx)
                                .get_dims();
                            Polygon::rectangle(dims.width, dims.height)
                        },
                    )
                    .build(ctx, "step forwards 0.1 seconds", hotkey(Key::M)),
                    Btn::svg_def("../data/system/assets/speed/jump_to_time.svg").build(
                        ctx,
                        "jump to specific time",
                        hotkey(Key::B),
                    ),
                    Btn::svg_def("../data/system/assets/speed/reset.svg").build(
                        ctx,
                        "reset to midnight",
                        hotkey(Key::X),
                    ),
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
                Widget::row(row.into_iter().map(|x| x.margin(5)).collect()).bg(colors::PANEL_BG),
            )
            .aligned(
                HorizontalAlignment::Center,
                VerticalAlignment::BottomAboveOSD,
            )
            .build(ctx),
        )
        .cb(
            "step forwards 0.1 seconds",
            Box::new(|ctx, app| {
                app.primary
                    .sim
                    .normal_step(&app.primary.map, Duration::seconds(0.1));
                if let Some(ref mut s) = app.secondary {
                    s.sim.normal_step(&s.map, Duration::seconds(0.1));
                }
                app.recalculate_current_selection(ctx);
                None
            }),
        )
        .cb(
            "step forwards 1 hour",
            Box::new(|ctx, app| {
                Some(Transition::Push(Box::new(TimeWarpScreen::new(
                    ctx,
                    app,
                    app.primary.sim.time() + Duration::hours(1),
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
        app: &mut App,
        maybe_mode: Option<&GameplayMode>,
    ) -> Option<Transition> {
        match self.composite.event(ctx, app) {
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
                        app.primary.clear_sim();
                        return Some(Transition::Replace(Box::new(SandboxMode::new(
                            ctx,
                            app,
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
                        app,
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
                // TODO This should match the update frequency in ezgapp. Plumb along the deadline
                // or frequency to here.
                app.primary
                    .sim
                    .time_limited_step(&app.primary.map, dt, Duration::seconds(0.033));
                app.recalculate_current_selection(ctx);
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
}

impl JumpToTime {
    fn new(ctx: &mut EventCtx, app: &App, maybe_mode: Option<GameplayMode>) -> JumpToTime {
        let target = app.primary.sim.time();
        JumpToTime {
            target,
            maybe_mode,
            composite: Composite::new(
                Widget::col(vec![
                    Btn::text_fg("X")
                        .build_def(ctx, hotkey(Key::Escape))
                        .align_right(),
                    {
                        let mut txt = Text::from(Line("Jump to what time?").roboto_bold());
                        txt.add(Line(target.ampm_tostring()));
                        txt.draw(ctx)
                    }
                    .named("target time"),
                    Widget::slider({
                        // TODO Auto-fill width?
                        let mut slider =
                            Slider::horizontal(ctx, 0.25 * ctx.canvas.window_width, 25.0);
                        slider.set_percent(ctx, target.to_percent(Time::END_OF_DAY).min(1.0));
                        slider
                    })
                    .named("time slider")
                    .margin(10),
                    Widget::row(vec![
                        Line("00:00").size(12).roboto().draw(ctx),
                        Widget::draw_svg(ctx, "../data/system/assets/speed/sunrise.svg"),
                        Line("12:00").size(12).roboto().draw(ctx),
                        Widget::draw_svg(ctx, "../data/system/assets/speed/sunset.svg"),
                        Line("24:00").size(12).roboto().draw(ctx),
                    ])
                    .padding(10)
                    .evenly_spaced(),
                    Widget::checkbox(ctx, "Stop when there's a traffic jam", None, false)
                        .padding(10)
                        .margin(10),
                    Btn::text_bg2("Go!")
                        .build_def(ctx, hotkey(Key::Enter))
                        .centered_horiz(),
                    Line("Active agents").roboto_bold().draw(ctx),
                    // TODO Sync the slider / plot.
                    Plot::new_usize(
                        ctx,
                        vec![if app.has_prebaked().is_some() {
                            Series {
                                label: "Baseline".to_string(),
                                color: Color::BLUE,
                                pts: app.prebaked().active_agents(Time::END_OF_DAY),
                            }
                        } else {
                            Series {
                                label: "Current simulation".to_string(),
                                color: Color::RED,
                                pts: app
                                    .primary
                                    .sim
                                    .get_analytics()
                                    .active_agents(app.primary.sim.time()),
                            }
                        }],
                        PlotOptions {
                            max_x: Some(Time::END_OF_DAY),
                        },
                    ),
                ])
                .bg(colors::PANEL_BG),
            )
            .build(ctx),
        }
    }
}

impl State for JumpToTime {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "X" => {
                    return Transition::Pop;
                }
                "Go!" => {
                    let traffic_jams = self.composite.is_checked("Stop when there's a traffic jam");
                    if self.target < app.primary.sim.time() {
                        if let Some(mode) = self.maybe_mode.take() {
                            app.primary.clear_sim();
                            return Transition::ReplaceThenPush(
                                Box::new(SandboxMode::new(ctx, app, mode)),
                                Box::new(TimeWarpScreen::new(ctx, app, self.target, traffic_jams)),
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
                        app,
                        self.target,
                        traffic_jams,
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
                {
                    let mut txt = Text::from(Line("Jump to what time?").roboto_bold());
                    txt.add(Line(target.ampm_tostring()));
                    // TODO The panel jumps too much and the slider position changes place.
                    /*if target < app.primary.sim.time() {
                        txt.add(Line("(Going back in time will reset to midnight, then simulate forwards)"));
                    }*/
                    txt.draw(ctx)
                }
                .named("target time"),
            );
        }
        if self.composite.clicked_outside(ctx) {
            return Transition::Pop;
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
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
    fn new(ctx: &mut EventCtx, app: &mut App, target: Time, traffic_jams: bool) -> TimeWarpScreen {
        if traffic_jams {
            app.primary
                .sim
                .set_gridlock_checker(Some(Duration::minutes(5)));
        }

        TimeWarpScreen {
            target,
            started: Instant::now(),
            traffic_jams,
            composite: Composite::new(
                Widget::col(vec![
                    Text::new().draw(ctx).named("text"),
                    Btn::text_bg2("stop now")
                        .build_def(ctx, hotkey(Key::Escape))
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
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        if ctx.input.nonblocking_is_update_event().is_some() {
            ctx.input.use_update_event();
            if let Some(problems) = app.primary.sim.time_limited_step(
                &app.primary.map,
                self.target - app.primary.sim.time(),
                Duration::seconds(0.033),
            ) {
                let id = ID::Intersection(problems[0].0);
                app.overlay = Overlays::traffic_jams(ctx, app);
                return Transition::Replace(Warping::new(
                    ctx,
                    id.canonical_point(&app.primary).unwrap(),
                    Some(10.0),
                    Some(id),
                    &mut app.primary,
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
                app.primary.sim.time().ampm_tostring()
            )));
            txt.add(Line(format!(
                "Have been simulating for {}",
                Duration::realtime_elapsed(self.started)
            )));

            self.composite
                .replace(ctx, "text", txt.draw(ctx).named("text"));
        }
        if app.primary.sim.time() == self.target {
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

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        State::grey_out_map(g);
        self.composite.draw(g);
    }

    fn on_destroy(&mut self, _: &mut EventCtx, app: &mut App) {
        if self.traffic_jams {
            app.primary.sim.set_gridlock_checker(None);
        }
    }
}

pub struct TimePanel {
    time: Time,
    pub composite: Composite,
}

impl TimePanel {
    pub fn new(ctx: &mut EventCtx, app: &App) -> TimePanel {
        TimePanel {
            time: app.primary.sim.time(),
            composite: Composite::new(
                Widget::col(vec![
                    Text::from(Line(app.primary.sim.time().ampm_tostring()).size(30))
                        .draw(ctx)
                        .margin(10)
                        .centered_horiz(),
                    {
                        let mut batch = GeomBatch::new();
                        // This is manually tuned
                        let width = 300.0;
                        let height = 15.0;
                        // Just clamp past 24 hours
                        let percent = app.primary.sim.time().to_percent(Time::END_OF_DAY).min(1.0);

                        // TODO rounded
                        batch.push(Color::WHITE, Polygon::rectangle(width, height));
                        if percent != 0.0 {
                            batch.push(
                                colors::SECTION_BG,
                                Polygon::rectangle(percent * width, height),
                            );
                        }
                        Widget::draw_batch(ctx, batch)
                    },
                    Widget::row(vec![
                        Line("00:00").size(12).roboto().draw(ctx),
                        Widget::draw_svg(ctx, "../data/system/assets/speed/sunrise.svg"),
                        Line("12:00").size(12).roboto().draw(ctx),
                        Widget::draw_svg(ctx, "../data/system/assets/speed/sunset.svg"),
                        Line("24:00").size(12).roboto().draw(ctx),
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

    pub fn event(&mut self, ctx: &mut EventCtx, app: &mut App) {
        if self.time != app.primary.sim.time() {
            *self = TimePanel::new(ctx, app);
        }
        self.composite.event(ctx);
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        self.composite.draw(g);
    }
}
