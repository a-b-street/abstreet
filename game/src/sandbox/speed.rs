use crate::app::{App, FindDelayedIntersections, ShowEverything};
use crate::common::Warping;
use crate::game::{msg, DrawBaselayer, State, Transition};
use crate::helpers::ID;
use crate::render::DrawOptions;
use crate::sandbox::{GameplayMode, SandboxMode};
use abstutil::prettyprint_usize;
use ezgui::{
    hotkey, AreaSlider, Btn, Checkbox, Choice, Color, Composite, EventCtx, GeomBatch, GfxCtx,
    HorizontalAlignment, Key, Line, Outcome, PersistentSplit, RewriteColor, Text, UpdateType,
    VerticalAlignment, Widget,
};
use geom::{Duration, Polygon, Pt2D, Time};
use instant::Instant;
use sim::AlertLocation;

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
            Widget::custom_row(
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
            Widget::custom_row(vec![
                Btn::svg_def("../data/system/assets/speed/jump_to_time.svg")
                    .pad(9)
                    .build(ctx, "jump to specific time", hotkey(Key::B)),
                Btn::svg_def("../data/system/assets/speed/reset.svg")
                    .pad(9)
                    .build(ctx, "reset to midnight", hotkey(Key::X)),
            ])
            .bg(app.cs.section_bg),
        );

        Composite::new(Widget::custom_row(row))
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
                        app.primary
                            .sim
                            .tiny_step(&app.primary.map, &mut app.primary.sim_cb);
                        app.recalculate_current_selection(ctx);
                        return Some(Transition::KeepWithMouseover);
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
                // TODO This should match the update frequency in ezgui. Plumb along the deadline
                // or frequency to here.
                app.primary.sim.time_limited_step(
                    &app.primary.map,
                    dt,
                    Duration::seconds(0.033),
                    &mut app.primary.sim_cb,
                );
                app.recalculate_current_selection(ctx);
            }
        }

        // TODO Need to do this anywhere that steps the sim, like TimeWarpScreen.
        let alerts = app.primary.sim.clear_alerts();
        if !alerts.is_empty() {
            let popup = msg("Alerts", alerts.iter().map(|(_, _, msg)| msg).collect());
            let maybe_id = match alerts[0].1 {
                AlertLocation::Nil => None,
                AlertLocation::Intersection(i) => Some(ID::Intersection(i)),
                // TODO Open info panel and warp to them
                AlertLocation::Person(_) => None,
                AlertLocation::Building(b) => Some(ID::Building(b)),
            };
            // TODO Can filter for particular alerts places like this:
            /*if !alerts[0].2.contains("Turn conflict cycle") {
                return None;
            }*/
            /*if maybe_id != Some(ID::Building(map_model::BuildingID(91))) {
                return None;
            }*/
            self.pause(ctx, app);
            if let Some(id) = maybe_id {
                // Just go to the first one, but print all messages
                return Some(Transition::PushTwice(
                    popup,
                    Warping::new(
                        ctx,
                        id.canonical_point(&app.primary).unwrap(),
                        Some(10.0),
                        None,
                        &mut app.primary,
                    ),
                ));
            } else {
                return Some(Transition::Push(popup));
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
            composite: Composite::new(Widget::col(vec![
                Widget::row(vec![
                    Line("Jump to what time?").small_heading().draw(ctx),
                    Btn::plaintext("X")
                        .build(ctx, "close", hotkey(Key::Escape))
                        .align_right(),
                ]),
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
                AreaSlider::new(
                    ctx,
                    0.25 * ctx.canvas.window_width,
                    target.to_percent(end_of_day).min(1.0),
                )
                .named("time slider"),
                Btn::text_bg2(format!("Jump to {}", target.ampm_tostring()))
                    .build(ctx, "jump to time", hotkey(Key::Enter))
                    .centered_horiz()
                    .named("jump to time"),
                Widget::draw_batch(
                    ctx,
                    GeomBatch::from(vec![(
                        Color::WHITE,
                        Polygon::rectangle(0.25 * ctx.canvas.window_width, 2.0),
                    )]),
                )
                .margin_above(10),
                Btn::text_bg2("Jump to the next delay over 5 minutes")
                    .build_def(ctx, None)
                    .centered_horiz(),
                Checkbox::text(
                    ctx,
                    "don't draw (for faster simulations)",
                    None,
                    app.opts.dont_draw_time_warp,
                )
                .margin_above(30)
                .named("don't draw"),
            ]))
            .build(ctx),
        }
    }
}

impl State for JumpToTime {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                "jump to time" => {
                    if self.target < app.primary.sim.time() {
                        if let Some(mode) = self.maybe_mode.take() {
                            return Transition::ReplaceThenPush(
                                Box::new(SandboxMode::new(ctx, app, mode)),
                                TimeWarpScreen::new(ctx, app, self.target, false),
                            );
                        } else {
                            return Transition::Replace(msg(
                                "Error",
                                vec!["Sorry, you can't go rewind time from this mode."],
                            ));
                        }
                    }
                    return Transition::Replace(TimeWarpScreen::new(ctx, app, self.target, false));
                }
                "Jump to the next delay over 5 minutes" => {
                    return Transition::Replace(TimeWarpScreen::new(
                        ctx,
                        app,
                        app.primary.sim.get_end_of_day(),
                        true,
                    ));
                }
                _ => unreachable!(),
            },
            None => {}
        }
        app.opts.dont_draw_time_warp = self.composite.is_checked("don't draw");
        let target = app
            .primary
            .sim
            .get_end_of_day()
            .percent_of(self.composite.area_slider("time slider").get_percent())
            .round_seconds(600.0);
        if target != self.target {
            self.target = target;
            self.composite.replace(
                ctx,
                "jump to time",
                Btn::text_bg2(format!("Jump to {}", target.ampm_tostring()))
                    .build(ctx, "jump to time", hotkey(Key::Enter))
                    .centered_horiz()
                    .named("jump to time"),
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
        mut traffic_jams: bool,
    ) -> Box<dyn State> {
        if traffic_jams {
            if app.primary.sim_cb.is_none() {
                app.primary.sim_cb = Some(Box::new(FindDelayedIntersections {
                    halt_limit: Duration::minutes(5),
                    report_limit: Duration::minutes(5),
                    currently_delayed: Vec::new(),
                }));
                // TODO Can we get away with less frequently? Not sure about all the edge cases
                app.primary.sim.set_periodic_callback(Duration::minutes(1));
            } else {
                traffic_jams = false;
            }
        }

        Box::new(TimeWarpScreen {
            target,
            started: Instant::now(),
            traffic_jams,
            composite: Composite::new(Widget::col(vec![
                Text::new().draw(ctx).named("text"),
                Btn::text_bg2("stop now")
                    .build_def(ctx, hotkey(Key::Escape))
                    .centered_horiz(),
            ]))
            .build(ctx),
        })
    }
}

impl State for TimeWarpScreen {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        if ctx.input.nonblocking_is_update_event().is_some() {
            ctx.input.use_update_event();
            app.primary.sim.time_limited_step(
                &app.primary.map,
                self.target - app.primary.sim.time(),
                Duration::seconds(0.033),
                &mut app.primary.sim_cb,
            );
            for (t, maybe_i, alert) in app.primary.sim.clear_alerts() {
                // TODO Just the first :(
                return Transition::Replace(msg(
                    "Alert",
                    vec![format!("At {}, near {:?}, {}", t, maybe_i, alert)],
                ));
            }
            if let Some(ref mut cb) = app.primary.sim_cb {
                let di = cb.downcast_mut::<FindDelayedIntersections>().unwrap();
                if let Some((i, t)) = di.currently_delayed.get(0) {
                    if app.primary.sim.time() - *t > di.halt_limit {
                        let id = ID::Intersection(*i);
                        app.layer =
                            Some(Box::new(crate::layer::traffic::TrafficJams::new(ctx, app)));
                        return Transition::Replace(Warping::new(
                            ctx,
                            id.canonical_point(&app.primary).unwrap(),
                            Some(10.0),
                            Some(id),
                            &mut app.primary,
                        ));
                    }
                }
            }

            let now = app.primary.sim.time();
            let (finished_after, _, _) = app.primary.sim.num_trips();
            let finished_before = if app.has_prebaked().is_some() {
                let mut cnt = 0;
                for (t, _, _, _) in &app.prebaked().finished_trips {
                    if *t > now {
                        break;
                    }
                    cnt += 1;
                }
                Some(cnt)
            } else {
                None
            };
            let txt = Text::from_multiline(vec![
                // I'm covered in shame for not doing this from the start.
                Line("Let's do the time warp again!").small_heading(),
                Line(format!(
                    "{} / {}",
                    now.ampm_tostring(),
                    self.target.ampm_tostring()
                )),
                Line(format!(
                    "Elapsed simulation time: {}",
                    Duration::realtime_elapsed(self.started)
                )),
                if let Some(n) = finished_before {
                    // TODO Underline
                    Line(format!(
                        "Finished trips: {} ({} compared to before \"{}\")",
                        prettyprint_usize(finished_after),
                        compare_count(finished_after, n),
                        app.primary.map.get_edits().edits_name,
                    ))
                } else {
                    Line(format!(
                        "Finished trips: {}",
                        prettyprint_usize(finished_after)
                    ))
                },
            ]);

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

        ctx.request_update(UpdateType::Game);
        Transition::Keep
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::Custom
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        if app.opts.dont_draw_time_warp {
            g.clear(app.cs.section_bg);
        } else {
            app.draw(
                g,
                DrawOptions::new(),
                &app.primary.sim,
                &ShowEverything::new(),
            );
            State::grey_out_map(g, app);
        }

        self.composite.draw(g);
    }

    fn on_destroy(&mut self, _: &mut EventCtx, app: &mut App) {
        if self.traffic_jams {
            assert!(app.primary.sim_cb.is_some());
            app.primary.sim_cb = None;
            app.primary.sim.unset_periodic_callback();
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
            composite: Composite::new(Widget::col(vec![
                Text::from(
                    Line(app.primary.sim.time().ampm_tostring_spacers()).big_heading_styled(),
                )
                .draw(ctx)
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
                Widget::custom_row(vec![
                    Line("00:00").small().draw(ctx),
                    Widget::draw_svg(ctx, "../data/system/assets/speed/sunrise.svg"),
                    Line("12:00").small().draw(ctx),
                    Widget::draw_svg(ctx, "../data/system/assets/speed/sunset.svg"),
                    Line("24:00").small().draw(ctx),
                ])
                .evenly_spaced(),
            ]))
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
        pts.push(lttb::DataPoint::new(
            width * (t - min_x) / (max_x - min_x),
            height * (1.0 - (((cnt - min_y) as f64) / ((max_y - min_y) as f64))),
        ));
    }
    let mut downsampled = Vec::new();
    for pt in lttb::lttb(pts, 100) {
        downsampled.push(Pt2D::new(pt.x, pt.y));
    }
    downsampled.push(downsampled[0]);
    Polygon::new(&downsampled)
}

// TODO Maybe color, put in helpers
fn compare_count(after: usize, before: usize) -> String {
    if after == before {
        "+0".to_string()
    } else if after > before {
        format!("+{}", prettyprint_usize(after - before))
    } else {
        format!("-{}", prettyprint_usize(before - after))
    }
}
