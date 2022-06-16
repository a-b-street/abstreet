use abstutil::prettyprint_usize;
use geom::{Circle, Distance, Duration, Polygon, Pt2D, Ring, Time};
use map_gui::ID;
use sim::AlertLocation;
use widgetry::tools::PopupMsg;
use widgetry::{
    Choice, Color, ControlState, DrawWithTooltips, EdgeInsets, EventCtx, GeomBatch, GfxCtx,
    HorizontalAlignment, Key, Line, Outcome, Panel, PanelDims, PersistentSplit, ScreenDims, Text,
    TextExt, VerticalAlignment, Widget,
};

use crate::app::{App, Transition};
use crate::common::Warping;
use crate::sandbox::time_warp::JumpToTime;
use crate::sandbox::{GameplayMode, SandboxMode, TimeWarpScreen};

pub struct TimePanel {
    pub panel: Panel,
    pub override_height: Option<f64>,

    time: Time,
    paused: bool,
    setting: SpeedSetting,
    // if present, how many trips were completed in the baseline at this point
    baseline_finished_trips: Option<usize>,
}

#[derive(Clone, Copy, PartialEq, PartialOrd)]
pub enum SpeedSetting {
    /// 1 sim second per real second
    Realtime,
    /// 5 sim seconds per real second
    Fast,
    /// 30 sim seconds per real second
    Faster,
    /// 1 sim hour per real second
    Fastest,
}

impl TimePanel {
    pub fn new(ctx: &mut EventCtx, app: &App) -> TimePanel {
        let mut time = TimePanel {
            panel: Panel::empty(ctx),
            override_height: None,
            time: app.primary.sim.time(),
            paused: false,
            setting: SpeedSetting::Realtime,
            baseline_finished_trips: None,
        };
        time.recreate_panel(ctx, app);
        time
    }

    pub fn recreate_panel(&mut self, ctx: &mut EventCtx, app: &App) {
        let mut row = Vec::new();
        row.push({
            let button = ctx
                .style()
                .btn_plain
                .icon("system/assets/speed/triangle.svg")
                .hotkey(Key::Space);

            Widget::custom_row(vec![if self.paused {
                button.build_widget(ctx, "play")
            } else {
                button
                    .image_path("system/assets/speed/pause.svg")
                    .build_widget(ctx, "pause")
            }])
            .margin_right(16)
        });

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
                    txt.extend(Text::tooltip(ctx, Key::LeftArrow, "slow down"));
                    txt.extend(Text::tooltip(ctx, Key::RightArrow, "speed up"));

                    let mut triangle_btn = ctx
                        .style()
                        .btn_plain
                        .btn()
                        .image_path("system/assets/speed/triangle.svg")
                        .image_dims(ScreenDims::new(16.0, 26.0))
                        .tooltip(txt)
                        .padding(EdgeInsets {
                            top: 8.0,
                            bottom: 8.0,
                            left: 3.0,
                            right: 3.0,
                        });

                    if s == SpeedSetting::Realtime {
                        triangle_btn = triangle_btn.padding_left(10.0);
                    }
                    if s == SpeedSetting::Fastest {
                        triangle_btn = triangle_btn.padding_right(10.0);
                    }

                    if self.setting < s {
                        triangle_btn = triangle_btn
                            .image_color(ctx.style().btn_outline.fg_disabled, ControlState::Default)
                    }

                    triangle_btn.build_widget(ctx, label)
                })
                .collect(),
            )
            .margin_right(16),
        );

        row.push(
            PersistentSplit::widget(
                ctx,
                "step forwards",
                app.opts.time_increment,
                Key::M,
                vec![
                    Choice::new("+1h", Duration::hours(1)),
                    Choice::new("+30m", Duration::minutes(30)),
                    Choice::new("+10m", Duration::minutes(10)),
                    Choice::new("+0.1s", Duration::seconds(0.1)),
                ],
            )
            .margin_right(16),
        );

        row.push(
            ctx.style()
                .btn_plain
                .icon("system/assets/speed/jump_to_time.svg")
                .hotkey(Key::B)
                .build_widget(ctx, "jump to specific time"),
        );

        row.push(
            ctx.style()
                .btn_plain
                .icon("system/assets/speed/reset.svg")
                .hotkey(Key::X)
                .build_widget(ctx, "reset to midnight"),
        );

        let mut panel = Panel::new_builder(Widget::col(vec![
            self.create_time_panel(ctx, app).named("time"),
            Widget::custom_row(row),
        ]))
        .aligned(HorizontalAlignment::Left, VerticalAlignment::Top);
        if let Some(h) = self.override_height {
            panel = panel.dims_height(PanelDims::ExactPixels(h));
        }
        self.panel = panel.build(ctx);
    }

    fn trips_completion_bar(&mut self, ctx: &EventCtx, app: &App) -> Widget {
        let text_color = Color::WHITE;
        let bar_fg = ctx.style().primary_fg;
        let bar_bg = bar_fg.tint(0.6).shade(0.2);
        let cursor_fg = Color::hex("#939393");

        // This is manually tuned
        let bar_width = 400.0;
        let bar_height = 27.0;

        let (finished, unfinished) = app.primary.sim.num_trips();
        let total = finished + unfinished;
        let ratio = if total > 0 {
            finished as f64 / total as f64
        } else {
            0.0
        };
        let finished_width = ratio * bar_width;

        if app.has_prebaked().is_some() {
            let now = self.time;
            let mut baseline_finished = self.baseline_finished_trips.unwrap_or(0);
            for (t, _, _, _) in &app.prebaked().finished_trips[baseline_finished..] {
                if *t > now {
                    break;
                }
                baseline_finished += 1;
            }
            // memoized for perf.
            // A bit of profiling shows we save about 0.7% of runtime
            // (using montlake, zoomed out, at max speed)
            self.baseline_finished_trips = Some(baseline_finished);
        }

        let baseline_finished_ratio: Option<f64> =
            self.baseline_finished_trips.and_then(|baseline_finished| {
                if unfinished + baseline_finished > 0 {
                    Some(baseline_finished as f64 / (baseline_finished + unfinished) as f64)
                } else {
                    None
                }
            });
        let baseline_finished_width: Option<f64> = baseline_finished_ratio
            .map(|baseline_finished_ratio| baseline_finished_ratio * bar_width);

        let cursor_width = 2.0;
        let mut progress_bar = GeomBatch::new();

        {
            // TODO Why is the rounding so hard? The white background is always rounded
            // at both ends. The moving bar should always be rounded on the left, flat
            // on the right, except at the very end (for the last 'radius' pixels). And
            // when the width is too small for the radius, this messes up.
            progress_bar.push(bar_bg, Polygon::rectangle(bar_width, bar_height));
            progress_bar.push(bar_fg, Polygon::rectangle(finished_width, bar_height));

            if let Some(baseline_finished_width) = baseline_finished_width {
                if baseline_finished_width > 0.0 {
                    let baseline_cursor = Polygon::rectangle(cursor_width, bar_height)
                        .translate(baseline_finished_width, 0.0);
                    progress_bar.push(cursor_fg, baseline_cursor);
                }
            }
        }

        let text_geom = Text::from(
            Line(format!("Finished Trips: {}", prettyprint_usize(finished))).fg(text_color),
        )
        .render(ctx)
        .translate(8.0, 0.0);
        progress_bar.append(text_geom);

        if let Some(baseline_finished_width) = baseline_finished_width {
            let triangle_width = 9.0;
            let triangle_height = 9.0;

            // Add a triangle-shaped cursor above the baseline cursor
            progress_bar = progress_bar.translate(0.0, triangle_height);

            let triangle = Ring::must_new(vec![
                Pt2D::zero(),
                Pt2D::new(triangle_width, 0.0),
                Pt2D::new(triangle_width / 2.0, triangle_height),
                Pt2D::zero(),
            ])
            .into_polygon()
            .translate(
                baseline_finished_width - triangle_width / 2.0 + cursor_width / 2.0,
                0.0,
            );
            progress_bar.push(cursor_fg, triangle);
        }

        let mut tooltip_text = Text::from("Finished Trips");
        tooltip_text.add_line(format!(
            "{} ({}% of total)",
            prettyprint_usize(finished),
            (ratio * 100.0) as usize
        ));
        if let Some(baseline_finished) = self.baseline_finished_trips {
            // TODO: up/down icons
            let line = match baseline_finished.cmp(&finished) {
                std::cmp::Ordering::Greater => {
                    let difference = baseline_finished - finished;
                    Line(format!(
                        "{} less than baseline",
                        prettyprint_usize(difference)
                    ))
                    .fg(ctx.style().text_destructive_color)
                }
                std::cmp::Ordering::Less => {
                    let difference = finished - baseline_finished;
                    Line(format!(
                        "{} more than baseline",
                        prettyprint_usize(difference)
                    ))
                    .fg(Color::GREEN)
                }
                std::cmp::Ordering::Equal => Line("No change from baseline"),
            };
            tooltip_text.add_line(line);
        }

        let bounds = progress_bar.get_bounds();
        let bounding_box = Polygon::rectangle(bounds.width(), bounds.height());
        let tooltip = vec![(bounding_box, tooltip_text, None)];
        DrawWithTooltips::new_widget(ctx, progress_bar, tooltip, Box::new(|_| GeomBatch::new()))
    }

    fn create_time_panel(&mut self, ctx: &EventCtx, app: &App) -> Widget {
        let trips_bar = self.trips_completion_bar(ctx, app);

        // TODO This likely fits better in the top center panel, but no easy way to squeeze it
        // into the panel for all gameplay modes
        let record_trips = if let Some(n) = app.primary.sim.num_recorded_trips() {
            Widget::row(vec![
                GeomBatch::from(vec![(
                    Color::RED,
                    Circle::new(Pt2D::new(0.0, 0.0), Distance::meters(10.0)).to_polygon(),
                )])
                .into_widget(ctx)
                .centered_vert(),
                format!("{} trips captured", prettyprint_usize(n)).text_widget(ctx),
                ctx.style()
                    .btn_solid_primary
                    .text("Finish Capture")
                    .build_def(ctx)
                    .align_right(),
            ])
        } else {
            Widget::nothing()
        };

        Widget::col(vec![
            Text::from(Line(self.time.ampm_tostring()).big_monospaced()).into_widget(ctx),
            trips_bar.margin_above(12),
            if app.primary.dirty_from_edits {
                ctx.style()
                    .btn_plain
                    .icon("system/assets/tools/warning.svg")
                    .build_widget(ctx, "see why results are tentative")
                    .centered_vert()
                    .align_right()
            } else {
                Widget::nothing()
            },
            record_trips,
        ])
    }

    pub fn event(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        maybe_mode: Option<&GameplayMode>,
    ) -> Option<Transition> {
        if self.time != app.primary.sim.time() {
            self.time = app.primary.sim.time();
            let time = self.create_time_panel(ctx, app);
            self.panel.replace(ctx, "time", time);
        }

        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "real-time speed" => {
                    self.setting = SpeedSetting::Realtime;
                    self.recreate_panel(ctx, app);
                    return None;
                }
                "5x speed" => {
                    self.setting = SpeedSetting::Fast;
                    self.recreate_panel(ctx, app);
                    return None;
                }
                "30x speed" => {
                    self.setting = SpeedSetting::Faster;
                    self.recreate_panel(ctx, app);
                    return None;
                }
                "3600x speed" => {
                    self.setting = SpeedSetting::Fastest;
                    self.recreate_panel(ctx, app);
                    return None;
                }
                "play" => {
                    self.paused = false;
                    self.recreate_panel(ctx, app);
                    return None;
                }
                "pause" => {
                    self.pause(ctx, app);
                }
                "reset to midnight" => {
                    if let Some(mode) = maybe_mode {
                        return Some(Transition::Replace(SandboxMode::simple_new(
                            app,
                            mode.clone(),
                        )));
                    } else {
                        return Some(Transition::Push(PopupMsg::new_state(
                            ctx,
                            "Error",
                            vec!["Sorry, you can't go rewind time from this mode."],
                        )));
                    }
                }
                "jump to specific time" => {
                    return Some(Transition::Push(JumpToTime::new_state(
                        ctx,
                        app,
                        maybe_mode.cloned(),
                    )));
                }
                "step forwards" => {
                    let dt = self.panel.persistent_split_value("step forwards");
                    if dt == Duration::seconds(0.1) {
                        app.primary
                            .sim
                            .tiny_step(&app.primary.map, &mut app.primary.sim_cb);
                        app.recalculate_current_selection(ctx);
                        return Some(Transition::KeepWithMouseover);
                    }
                    return Some(Transition::Push(TimeWarpScreen::new_state(
                        ctx,
                        app,
                        app.primary.sim.time() + dt,
                        None,
                    )));
                }
                "see why results are tentative" => {
                    return Some(Transition::Push(PopupMsg::new_state(
                        ctx,
                        "Simulation results not finalized",
                        vec![
                            "You edited the map in the middle of the day.",
                            "Some trips may have been interrupted, and others might have made \
                            different decisions if they saw the new map from the start.",
                            "To get final results, reset to midnight and test your proposal over \
                            a full day.",
                        ],
                    )));
                }
                "Finish Capture" => {
                    app.primary.sim.save_recorded_traffic(&app.primary.map);
                }
                _ => unreachable!(),
            },
            Outcome::Changed(x) => {
                if x == "step forwards" {
                    app.opts.time_increment = self.panel.persistent_split_value("step forwards");
                }
            }
            _ => {}
        }

        if ctx.input.pressed(Key::LeftArrow) {
            match self.setting {
                SpeedSetting::Realtime => self.pause(ctx, app),
                SpeedSetting::Fast => {
                    self.setting = SpeedSetting::Realtime;
                    self.recreate_panel(ctx, app);
                }
                SpeedSetting::Faster => {
                    self.setting = SpeedSetting::Fast;
                    self.recreate_panel(ctx, app);
                }
                SpeedSetting::Fastest => {
                    self.setting = SpeedSetting::Faster;
                    self.recreate_panel(ctx, app);
                }
            }
        }
        if ctx.input.pressed(Key::RightArrow) {
            match self.setting {
                SpeedSetting::Realtime => {
                    if self.paused {
                        self.paused = false;
                    } else {
                        self.setting = SpeedSetting::Fast;
                    }
                    self.recreate_panel(ctx, app);
                }
                SpeedSetting::Fast => {
                    self.setting = SpeedSetting::Faster;
                    self.recreate_panel(ctx, app);
                }
                SpeedSetting::Faster => {
                    self.setting = SpeedSetting::Fastest;
                    self.recreate_panel(ctx, app);
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
                // TODO This should match the update frequency in widgetry. Plumb along the deadline
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
            let popup = PopupMsg::new_state(
                ctx,
                "Alerts",
                alerts.iter().map(|(_, _, msg)| msg).collect(),
            );
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
                return Some(Transition::Multi(vec![
                    Transition::Push(popup),
                    Transition::Push(Warping::new_state(
                        ctx,
                        app.primary.canonical_point(id).unwrap(),
                        Some(10.0),
                        None,
                        &mut app.primary,
                    )),
                ]));
            } else {
                return Some(Transition::Push(popup));
            }
        }

        None
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        self.panel.draw(g);
    }

    pub fn pause(&mut self, ctx: &mut EventCtx, app: &App) {
        if !self.paused {
            self.paused = true;
            self.recreate_panel(ctx, app);
        }
    }

    pub fn resume(&mut self, ctx: &mut EventCtx, app: &App, setting: SpeedSetting) {
        if self.paused || self.setting != setting {
            self.paused = false;
            self.setting = setting;
            self.recreate_panel(ctx, app);
        }
    }

    pub fn is_paused(&self) -> bool {
        self.paused
    }
}
