use crate::app::App;
use crate::common::Warping;
use crate::game::{msg, State, Transition};
use crate::helpers::ID;
use crate::sandbox::{GameplayMode, SandboxMode};
use ezgui::{
    hotkey, Btn, Checkbox, Choice, Color, Composite, EventCtx, EventLoopMode, GeomBatch, GfxCtx,
    HorizontalAlignment, Key, Line, Outcome, PersistentSplit, RewriteColor, Slider, Text,
    VerticalAlignment, Widget,
};
use geom::{Duration, PolyLine, Polygon, Pt2D, Time};
use instant::Instant;

pub struct SpeedControls {
    pub composite: Composite,

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
    // TODO Could use checkbox here, but not sure it'll make things that much simpler.
    fn make_panel(ctx: &mut EventCtx, app: &App, paused: bool, setting: SpeedSetting) -> Composite {
        let mut row = Vec::new();
        row.push(
            if paused {
                Btn::svg_def("../data/system/assets/speed/triangle.svg")
                    .pad(9)
                    .build(ctx, "play", hotkey(Key::Space))
            } else {
                Btn::svg_def("../data/system/assets/speed/pause.svg")
                    .pad(9)
                    .build(ctx, "pause", hotkey(Key::Space))
            }
            .bg(app.cs.section_bg)
            .margin_right(16),
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
                    let mut txt = Text::from(Line(label).small());
                    txt.extend(Text::tooltip(ctx, hotkey(Key::LeftArrow), "slow down"));
                    txt.extend(Text::tooltip(ctx, hotkey(Key::RightArrow), "speed up"));

                    Btn::svg_def("../data/system/assets/speed/triangle.svg")
                        .normal_color(if setting >= s {
                            RewriteColor::NoOp
                        } else {
                            RewriteColor::ChangeAll(Color::WHITE.alpha(0.2))
                        })
                        .pad(3)
                        .tooltip(txt)
                        .build(ctx, label, None)
                })
                .collect(),
            )
            .bg(app.cs.section_bg)
            .centered()
            .padding(6)
            .margin_right(16),
        );

        row.push(
            PersistentSplit::new(
                ctx,
                "step forwards",
                app.opts.time_increment,
                hotkey(Key::M),
                vec![
                    Choice::new("+1h", Duration::hours(1)),
                    Choice::new("+30m", Duration::minutes(30)),
                    Choice::new("+10m", Duration::minutes(10)),
                    Choice::new("+0.1s", Duration::seconds(0.1)),
                ],
            )
            .bg(app.cs.section_bg)
            .margin_right(16),
        );

        row.push(
            Widget::row(vec![
                Btn::svg_def("../data/system/assets/speed/jump_to_time.svg")
                    .pad(9)
                    .build(ctx, "jump to specific time", hotkey(Key::B)),
                Btn::svg_def("../data/system/assets/speed/reset.svg")
                    .pad(9)
                    .build(ctx, "reset to midnight", hotkey(Key::X)),
            ])
            .bg(app.cs.section_bg),
        );

        Composite::new(Widget::row(row).bg(app.cs.panel_bg).padding(16))
            .aligned(
                HorizontalAlignment::Center,
                VerticalAlignment::BottomAboveOSD,
            )
            .build(ctx)
    }

    pub fn new(ctx: &mut EventCtx, app: &App) -> SpeedControls {
        let composite = SpeedControls::make_panel(ctx, app, false, SpeedSetting::Realtime);
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
        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "real-time speed" => {
                    self.setting = SpeedSetting::Realtime;
                    self.composite = SpeedControls::make_panel(ctx, app, self.paused, self.setting);
                    return None;
                }
                "5x speed" => {
                    self.setting = SpeedSetting::Fast;
                    self.composite = SpeedControls::make_panel(ctx, app, self.paused, self.setting);
                    return None;
                }
                "30x speed" => {
                    self.setting = SpeedSetting::Faster;
                    self.composite = SpeedControls::make_panel(ctx, app, self.paused, self.setting);
                    return None;
                }
                "3600x speed" => {
                    self.setting = SpeedSetting::Fastest;
                    self.composite = SpeedControls::make_panel(ctx, app, self.paused, self.setting);
                    return None;
                }
                "play" => {
                    self.paused = false;
                    self.composite = SpeedControls::make_panel(ctx, app, self.paused, self.setting);
                    return None;
                }
                "pause" => {
                    self.pause(ctx, app);
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
                "step forwards" => {
                    let dt = self.composite.persistent_split_value("step forwards");
                    if dt == Duration::seconds(0.1) {
                        app.primary.sim.normal_step(&app.primary.map, dt);
                        if let Some(ref mut s) = app.secondary {
                            s.sim.normal_step(&s.map, dt);
                        }
                        app.recalculate_current_selection(ctx);
                        return None;
                    }
                    return Some(Transition::Push(TimeWarpScreen::new(
                        ctx,
                        app,
                        app.primary.sim.time() + dt,
                        false,
                    )));
                }
                _ => unreachable!(),
            },
            None => {}
        }
        // Just kind of constantly scrape this
        app.opts.time_increment = self.composite.persistent_split_value("step forwards");

        if ctx.input.new_was_pressed(&hotkey(Key::LeftArrow).unwrap()) {
            match self.setting {
                SpeedSetting::Realtime => self.pause(ctx, app),
                SpeedSetting::Fast => {
                    self.setting = SpeedSetting::Realtime;
                    self.composite = SpeedControls::make_panel(ctx, app, self.paused, self.setting);
                }
                SpeedSetting::Faster => {
                    self.setting = SpeedSetting::Fast;
                    self.composite = SpeedControls::make_panel(ctx, app, self.paused, self.setting);
                }
                SpeedSetting::Fastest => {
                    self.setting = SpeedSetting::Faster;
                    self.composite = SpeedControls::make_panel(ctx, app, self.paused, self.setting);
                }
            }
        }
        if ctx.input.new_was_pressed(&hotkey(Key::RightArrow).unwrap()) {
            match self.setting {
                SpeedSetting::Realtime => {
                    if self.paused {
                        self.paused = false;
                        self.composite =
                            SpeedControls::make_panel(ctx, app, self.paused, self.setting);
                    } else {
                        self.setting = SpeedSetting::Fast;
                        self.composite =
                            SpeedControls::make_panel(ctx, app, self.paused, self.setting);
                    }
                }
                SpeedSetting::Fast => {
                    self.setting = SpeedSetting::Faster;
                    self.composite = SpeedControls::make_panel(ctx, app, self.paused, self.setting);
                }
                SpeedSetting::Faster => {
                    self.setting = SpeedSetting::Fastest;
                    self.composite = SpeedControls::make_panel(ctx, app, self.paused, self.setting);
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

    pub fn pause(&mut self, ctx: &mut EventCtx, app: &App) {
        if !self.paused {
            self.paused = true;
            self.composite = SpeedControls::make_panel(ctx, app, self.paused, self.setting);
        }
    }

    pub fn resume_realtime(&mut self, ctx: &mut EventCtx, app: &App) {
        if self.paused || self.setting != SpeedSetting::Realtime {
            self.paused = false;
            self.setting = SpeedSetting::Realtime;
            self.composite = SpeedControls::make_panel(ctx, app, self.paused, self.setting);
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
        let end_of_day = app.primary.sim.get_end_of_day();
        JumpToTime {
            target,
            maybe_mode,
            composite: Composite::new(
                Widget::col(vec![
                    Btn::text_fg("X")
                        .build_def(ctx, hotkey(Key::Escape))
                        .align_right(),
                    {
                        let mut txt = Text::from(Line("Jump to what time?").small_heading());
                        txt.add(Line(target.ampm_tostring()));
                        txt.draw(ctx)
                    }
                    .named("target time"),
                    if app.has_prebaked().is_some() {
                        Widget::draw_batch(
                            ctx,
                            GeomBatch::from(vec![(
                                Color::WHITE.alpha(0.7),
                                area_under_curve(
                                    app.prebaked().active_agents(end_of_day),
                                    // TODO Auto fill width
                                    500.0,
                                    50.0,
                                ),
                            )]),
                        )
                    } else {
                        Widget::nothing()
                    },
                    // TODO Auto-fill width?
                    Slider::horizontal(
                        ctx,
                        0.25 * ctx.canvas.window_width,
                        25.0,
                        target.to_percent(end_of_day).min(1.0),
                    )
                    .named("time slider")
                    .margin(10),
                    Checkbox::text(ctx, "Stop when there's a traffic jam", None, false)
                        .padding(10)
                        .margin(10),
                    Btn::text_bg2("Go!")
                        .build_def(ctx, hotkey(Key::Enter))
                        .centered_horiz(),
                ])
                .bg(app.cs.panel_bg),
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
                                TimeWarpScreen::new(ctx, app, self.target, traffic_jams),
                            );
                        } else {
                            return Transition::Replace(msg(
                                "Error",
                                vec!["Sorry, you can't go rewind time from this mode."],
                            ));
                        }
                    }
                    return Transition::Replace(TimeWarpScreen::new(
                        ctx,
                        app,
                        self.target,
                        traffic_jams,
                    ));
                }
                _ => unreachable!(),
            },
            None => {}
        }
        let target = app
            .primary
            .sim
            .get_end_of_day()
            .percent_of(self.composite.slider("time slider").get_percent());
        if target != self.target {
            self.target = target;
            self.composite.replace(
                ctx,
                "target time",
                {
                    let mut txt = Text::from(Line("Jump to what time?").small_heading());
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

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        State::grey_out_map(g, app);
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
    pub fn new(
        ctx: &mut EventCtx,
        app: &mut App,
        target: Time,
        traffic_jams: bool,
    ) -> Box<dyn State> {
        if traffic_jams {
            app.primary
                .sim
                .set_gridlock_checker(Some(Duration::minutes(5)));
        }

        Box::new(TimeWarpScreen {
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
                .bg(app.cs.panel_bg),
            )
            .build(ctx),
        })
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
                app.layer = crate::layer::traffic::traffic_jams(ctx, app);
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
            let mut txt = Text::from(Line("Let's do the time warp again!").small_heading());
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

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        State::grey_out_map(g, app);
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
                    Text::from(
                        Line(app.primary.sim.time().ampm_tostring_spacers()).big_heading_styled(),
                    )
                    .draw(ctx)
                    .margin(10)
                    .centered_horiz(),
                    {
                        let mut batch = GeomBatch::new();
                        // This is manually tuned
                        let width = 300.0;
                        let height = 15.0;
                        // Just clamp if we simulate past the expected end
                        let percent = app
                            .primary
                            .sim
                            .time()
                            .to_percent(app.primary.sim.get_end_of_day())
                            .min(1.0);

                        // TODO Why is the rounding so hard? The white background is always rounded
                        // at both ends. The moving bar should always be rounded on the left, flat
                        // on the right, except at the very end (for the last 'radius' pixels). And
                        // when the width is too small for the radius, this messes up.

                        batch.push(Color::WHITE, Polygon::rectangle(width, height));

                        if percent != 0.0 {
                            batch.push(
                                if percent < 0.25 || percent > 0.75 {
                                    app.cs.night_time_slider
                                } else {
                                    app.cs.day_time_slider
                                },
                                Polygon::rectangle(percent * width, height),
                            );
                        }

                        Widget::draw_batch(ctx, batch)
                    },
                    Widget::row(vec![
                        Line("00:00").small().draw(ctx),
                        Widget::draw_svg(ctx, "../data/system/assets/speed/sunrise.svg"),
                        Line("12:00").small().draw(ctx),
                        Widget::draw_svg(ctx, "../data/system/assets/speed/sunset.svg"),
                        Line("24:00").small().draw(ctx),
                    ])
                    .padding(10)
                    .evenly_spaced(),
                ])
                .padding(10)
                .bg(app.cs.panel_bg),
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

fn area_under_curve(raw: Vec<(Time, usize)>, width: f64, height: f64) -> Polygon {
    assert!(!raw.is_empty());
    let min_x = Time::START_OF_DAY;
    let min_y = 0;
    let max_x = raw.last().unwrap().0;
    let max_y = raw.iter().max_by_key(|(_, cnt)| *cnt).unwrap().1;

    let mut pts = Vec::new();
    for (t, cnt) in raw {
        pts.push(Pt2D::new(
            (t - min_x) / (max_x - min_x) * width,
            ((cnt - min_y) as f64) / ((max_y - min_y) as f64) * height,
        ));
    }

    // TODO The smoothing should be tuned more
    let mut final_pts = PolyLine::new_simplified(pts, 5.0).into_points();
    final_pts.push(final_pts[0]);
    Polygon::new(&final_pts)
}
