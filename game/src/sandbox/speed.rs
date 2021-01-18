use geom::{Duration, Polygon, Time};
use map_gui::theme::StyledButtons;
use map_gui::tools::PopupMsg;
use map_gui::ID;
use sim::AlertLocation;
use widgetry::{
    Choice, Color, ControlState, EdgeInsets, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key,
    Line, Outcome, Panel, PersistentSplit, ScreenDims, Text, VerticalAlignment, Widget,
};

use crate::app::{App, Transition};
use crate::common::Warping;
use crate::sandbox::time_warp::JumpToTime;
use crate::sandbox::{GameplayMode, SandboxMode, TimeWarpScreen};

pub struct SpeedControls {
    pub panel: Panel,

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
    pub fn new(ctx: &mut EventCtx, app: &App) -> SpeedControls {
        let mut speed = SpeedControls {
            panel: Panel::empty(ctx),
            paused: false,
            setting: SpeedSetting::Realtime,
        };
        speed.recreate_panel(ctx, app);
        speed
    }

    pub fn recreate_panel(&mut self, ctx: &mut EventCtx, app: &App) {
        let mut row = Vec::new();
        row.push({
            let button = app
                .cs
                .btn_primary_light_icon("system/assets/speed/triangle.svg")
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

                    let mut triangle_btn = app
                        .cs
                        .btn_plain_light()
                        .image_path("system/assets/speed/triangle.svg")
                        .image_dims(ScreenDims::new(16.0, 22.0))
                        .bg_color(
                            app.cs.gui_style.btn_primary_light.bg_hover,
                            ControlState::Hover,
                        )
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
                        triangle_btn = triangle_btn.image_color(
                            app.cs.gui_style.btn_secondary_light.fg_disabled,
                            ControlState::Default,
                        )
                    }

                    triangle_btn.build_widget(ctx, label)
                })
                .collect(),
            )
            // Inner buttons, styled as one composite button w/ background/border
            .bg(app.cs.gui_style.btn_primary_light.bg)
            .outline(2.0, app.cs.gui_style.btn_primary_light.outline)
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
            {
                let buttons = app
                    .cs
                    .btn_plain_light()
                    .bg_color(
                        app.cs.gui_style.btn_primary_light.bg_hover,
                        ControlState::Hover,
                    )
                    .font_size(18)
                    .image_dims(ScreenDims::square(20.0));
                Widget::custom_row(vec![
                    buttons
                        .clone()
                        .label_text("jump")
                        .image_path("system/assets/speed/jump_to_time.svg")
                        .hotkey(Key::B)
                        .build_widget(ctx, "jump to specific time"),
                    buttons
                        .label_text("reset")
                        .image_path("system/assets/speed/reset.svg")
                        .hotkey(Key::X)
                        .build_widget(ctx, "reset to midnight"),
                ])
            }
            // Inner buttons, styled as one composite button w/ background/border
            .bg(app.cs.gui_style.btn_primary_light.bg)
            .outline(2.0, app.cs.gui_style.btn_primary_light.outline),
        );

        self.panel = Panel::new(Widget::custom_row(row))
            .aligned(
                HorizontalAlignment::Center,
                VerticalAlignment::BottomAboveOSD,
            )
            .build(ctx);
    }

    pub fn event(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        maybe_mode: Option<&GameplayMode>,
    ) -> Option<Transition> {
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
                            ctx,
                            app,
                            mode.clone(),
                        )));
                    } else {
                        return Some(Transition::Push(PopupMsg::new(
                            ctx,
                            "Error",
                            vec!["Sorry, you can't go rewind time from this mode."],
                        )));
                    }
                }
                "jump to specific time" => {
                    return Some(Transition::Push(JumpToTime::new(
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
                    return Some(Transition::Push(TimeWarpScreen::new(
                        ctx,
                        app,
                        app.primary.sim.time() + dt,
                        None,
                    )));
                }
                _ => unreachable!(),
            },
            _ => {}
        }
        // Just kind of constantly scrape this
        app.opts.time_increment = self.panel.persistent_split_value("step forwards");

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
                        self.recreate_panel(ctx, app);
                    } else {
                        self.setting = SpeedSetting::Fast;
                        self.recreate_panel(ctx, app);
                    }
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
            let popup = PopupMsg::new(
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
                    Transition::Push(Warping::new(
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

    pub fn resume_realtime(&mut self, ctx: &mut EventCtx, app: &App) {
        if self.paused || self.setting != SpeedSetting::Realtime {
            self.paused = false;
            self.setting = SpeedSetting::Realtime;
            self.recreate_panel(ctx, app);
        }
    }

    pub fn is_paused(&self) -> bool {
        self.paused
    }
}

pub struct TimePanel {
    time: Time,
    pub panel: Panel,
}

impl TimePanel {
    pub fn new(ctx: &mut EventCtx, app: &App) -> TimePanel {
        TimePanel {
            time: app.primary.sim.time(),
            panel: Panel::new(Widget::col(vec![
                Text::from(Line(app.primary.sim.time().ampm_tostring()).big_monospaced())
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
                    Line("00:00").small_monospaced().draw(ctx),
                    Widget::draw_svg(ctx, "system/assets/speed/sunrise.svg"),
                    Line("12:00").small_monospaced().draw(ctx),
                    Widget::draw_svg(ctx, "system/assets/speed/sunset.svg"),
                    Line("24:00").small_monospaced().draw(ctx),
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
        self.panel.event(ctx);
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        self.panel.draw(g);
    }
}
