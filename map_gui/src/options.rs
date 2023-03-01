use serde::{Deserialize, Serialize};

use abstutil::Timer;
use geom::{Duration, UnitFmt};
use widgetry::{
    CanvasSettings, Choice, EventCtx, GeomBatch, GfxCtx, Key, Line, Outcome, Panel, Spinner, State,
    TextExt, Toggle, Widget,
};

use crate::colors::ColorSchemeChoice;
use crate::render::DrawBuilding;
use crate::tools::grey_out_map;
use crate::AppLike;

/// Options controlling the UI. Some of the options are common to all map-based apps, and some are
/// specific to A/B Street.
// TODO SimOptions stuff too
#[derive(Clone, Serialize, Deserialize)]
pub struct Options {
    /// Dev mode exposes experimental tools useful for debugging, but that'd likely confuse most
    /// players.
    pub dev: bool,
    /// Every time we draw, render all agents zoomed in. Extremely slow. Just used to flush out
    /// drawing bugs.
    pub debug_all_agents: bool,

    /// How traffic signals should be rendered.
    pub traffic_signal_style: TrafficSignalStyle,
    /// The color scheme for map elements, agents, and the UI.
    pub color_scheme: ColorSchemeChoice,
    /// Automatically change color_scheme based on simulation time to reflect day/night
    pub toggle_day_night_colors: bool,
    /// Draw buildings in different perspectives
    pub camera_angle: CameraAngle,
    /// Draw building driveways.
    pub show_building_driveways: bool,
    /// Draw building outlines.
    pub show_building_outlines: bool,
    /// Draw stop signs.
    pub show_stop_signs: bool,
    /// Draw crosswalks and unmarked crossings.
    pub show_crosswalks: bool,
    /// If true, draw an icon for traffic signals both when zoomed and unzoomed. If false, color
    /// the intersection when unzoomed and render the signal's current state when zoomed.
    pub show_traffic_signal_icon: bool,
    /// If true, modify several basemap features to de-emphasize them: border intersections
    pub simplify_basemap: bool,

    /// When making a screen recording, enable this option to hide some UI elements
    pub minimal_controls: bool,
    /// widgetry options
    pub canvas_settings: CanvasSettings,

    /// How much to advance the sim with one of the speed controls
    pub time_increment: Duration,
    /// When time warping, don't draw to speed up simulation
    pub dont_draw_time_warp: bool,
    /// The delay threshold to halt on when jumping to the next delay
    pub jump_to_delay: Duration,

    /// Display roads and buildings in an alternate language, if possible. None means to use the
    /// OSM native name.
    pub language: Option<String>,
    /// How to render geometric units
    pub units: UnitFmt,
}

impl Options {
    /// Restore previous options. If the file is missing or the format has changed, fall back to
    /// built-in defaults.
    pub fn load_or_default() -> Options {
        match abstio::maybe_read_json::<Options>(
            abstio::path_player("settings.json"),
            &mut Timer::throwaway(),
        ) {
            Ok(opts) => {
                return opts;
            }
            Err(err) => {
                warn!("Couldn't restore settings, so using defaults. {}", err);
            }
        }

        Options {
            dev: false,
            debug_all_agents: false,

            traffic_signal_style: TrafficSignalStyle::Brian,
            color_scheme: ColorSchemeChoice::DayMode,
            toggle_day_night_colors: false,
            camera_angle: CameraAngle::TopDown,
            show_building_driveways: true,
            show_building_outlines: true,
            show_stop_signs: true,
            show_crosswalks: true,
            show_traffic_signal_icon: false,
            simplify_basemap: false,

            time_increment: Duration::minutes(10),
            dont_draw_time_warp: false,
            jump_to_delay: Duration::minutes(5),

            minimal_controls: false,
            canvas_settings: CanvasSettings::new(),
            language: None,
            units: UnitFmt {
                round_durations: true,
                // TODO Should default be based on the map?
                metric: false,
            },
        }
    }
}

/// Different ways of drawing traffic signals. The names of these aren't super meaningful...
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum TrafficSignalStyle {
    Brian,
    Yuwen,
    IndividualTurnArrows,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum CameraAngle {
    TopDown,
    IsometricNE,
    IsometricNW,
    IsometricSE,
    IsometricSW,
    Abstract,
}

pub struct OptionsPanel {
    panel: Panel,
}

impl OptionsPanel {
    pub fn new_state<A: AppLike>(ctx: &mut EventCtx, app: &A) -> Box<dyn State<A>> {
        Box::new(OptionsPanel {
            panel: Panel::new_builder(Widget::col(vec![
                Widget::custom_row(vec![
                    Line("Settings").small_heading().into_widget(ctx),
                    ctx.style().btn_close_widget(ctx),
                ]),
                "Camera controls".text_widget(ctx),
                Widget::col(vec![
                    Toggle::checkbox(
                        ctx,
                        "Invert direction of vertical scrolling",
                        None,
                        ctx.canvas.settings.invert_scroll,
                    ),
                    Toggle::checkbox(
                        ctx,
                        "Pan map when cursor is at edge of screen",
                        None,
                        ctx.canvas.settings.edge_auto_panning,
                    )
                    .named("autopan"),
                    Toggle::checkbox(
                        ctx,
                        "Use touchpad to pan and hold Control to zoom",
                        None,
                        ctx.canvas.settings.touchpad_to_move,
                    ),
                    Toggle::checkbox(
                        ctx,
                        "Use arrow keys to pan and Q/W to zoom",
                        None,
                        ctx.canvas.settings.keys_to_pan,
                    ),
                    Widget::row(vec![
                        "Scroll speed for menus".text_widget(ctx).centered_vert(),
                        Spinner::widget(
                            ctx,
                            "gui_scroll_speed",
                            (1, 50),
                            ctx.canvas.settings.gui_scroll_speed,
                            1,
                        ),
                    ]),
                    Widget::row(vec![
                        "Zoom speed for the map".text_widget(ctx).centered_vert(),
                        Spinner::widget(
                            ctx,
                            "canvas_scroll_speed",
                            (1, 30),
                            ctx.canvas.settings.canvas_scroll_speed,
                            1,
                        ),
                    ]),
                ])
                .bg(app.cs().inner_panel_bg)
                .padding(8),
                "Appearance".text_widget(ctx),
                Widget::col(vec![
                    Widget::row(vec![
                        "Traffic signal rendering:".text_widget(ctx),
                        Widget::dropdown(
                            ctx,
                            "Traffic signal rendering",
                            app.opts().traffic_signal_style.clone(),
                            vec![
                                Choice::new("Default (Brian's style)", TrafficSignalStyle::Brian),
                                Choice::new("Yuwen's style", TrafficSignalStyle::Yuwen),
                                Choice::new(
                                    "arrows showing individual turns (to debug)",
                                    TrafficSignalStyle::IndividualTurnArrows,
                                ),
                            ],
                        ),
                    ]),
                    Widget::row(vec![
                        "Camera angle:".text_widget(ctx),
                        Widget::dropdown(
                            ctx,
                            "Camera angle",
                            app.opts().camera_angle.clone(),
                            vec![
                                Choice::new("Top-down", CameraAngle::TopDown),
                                Choice::new("Isometric (northeast)", CameraAngle::IsometricNE),
                                Choice::new("Isometric (northwest)", CameraAngle::IsometricNW),
                                Choice::new("Isometric (southeast)", CameraAngle::IsometricSE),
                                Choice::new("Isometric (southwest)", CameraAngle::IsometricSW),
                                Choice::new("Abstract (just symbols)", CameraAngle::Abstract),
                            ],
                        ),
                    ]),
                    Widget::row(vec![
                        "Color scheme:".text_widget(ctx),
                        Widget::dropdown(
                            ctx,
                            "Color scheme",
                            app.opts().color_scheme,
                            ColorSchemeChoice::choices(),
                        ),
                    ]),
                    Widget::row(vec![
                        "Camera zoom to switch to unzoomed view".text_widget(ctx),
                        Widget::dropdown(
                            ctx,
                            "min zoom",
                            ctx.canvas.settings.min_zoom_for_detail,
                            vec![
                                Choice::new("1.0", 1.0),
                                Choice::new("2.0", 2.0),
                                Choice::new("3.0", 3.0),
                                Choice::new("4.0", 4.0),
                                Choice::new("5.0", 5.0),
                                Choice::new("6.0", 6.0),
                            ],
                        ),
                    ]),
                    Widget::row(vec!["Language".text_widget(ctx), {
                        let mut default = app.opts().language.clone();
                        let mut have_default = false;
                        let mut choices = vec![Choice::new("Map native language", None)];
                        for lang in app.map().get_languages() {
                            if default.as_ref() == Some(&lang) {
                                have_default = true;
                            }
                            choices.push(Choice::new(lang.clone(), Some(lang)));
                        }
                        // We might be switching from a map that has more languages than this
                        // map
                        if !have_default {
                            default = None;
                        }
                        Widget::dropdown(ctx, "language", default, choices)
                    }]),
                    Toggle::choice(
                        ctx,
                        "metric / imperial units",
                        "metric",
                        "imperial",
                        None,
                        app.opts().units.metric,
                    ),
                ])
                .bg(app.cs().inner_panel_bg)
                .padding(8),
                "Debug".text_widget(ctx),
                Widget::col(vec![
                    Toggle::checkbox(ctx, "Enable developer mode", None, app.opts().dev),
                    Toggle::checkbox(
                        ctx,
                        "Draw all agents to debug geometry (Slow!)",
                        None,
                        app.opts().debug_all_agents,
                    ),
                ])
                .bg(app.cs().inner_panel_bg)
                .padding(8),
                ctx.style()
                    .btn_solid_primary
                    .text("Apply")
                    .hotkey(Key::Enter)
                    .build_def(ctx)
                    .centered_horiz(),
            ]))
            .build(ctx),
        })
    }
}

impl<A: AppLike> State<A> for OptionsPanel {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut A) -> widgetry::Transition<A> {
        if let Outcome::Clicked(x) = self.panel.event(ctx) {
            match x.as_ref() {
                "close" => {
                    return widgetry::Transition::Pop;
                }
                "Apply" => {
                    let mut opts = app.opts().clone();
                    opts.dev = self.panel.is_checked("Enable developer mode");
                    opts.debug_all_agents = self
                        .panel
                        .is_checked("Draw all agents to debug geometry (Slow!)");

                    ctx.canvas.settings.invert_scroll = self
                        .panel
                        .is_checked("Invert direction of vertical scrolling");
                    ctx.canvas.settings.touchpad_to_move = self
                        .panel
                        .is_checked("Use touchpad to pan and hold Control to zoom");
                    ctx.canvas.settings.keys_to_pan = self
                        .panel
                        .is_checked("Use arrow keys to pan and Q/W to zoom");
                    ctx.canvas.settings.edge_auto_panning = self.panel.is_checked("autopan");
                    ctx.canvas.settings.gui_scroll_speed = self.panel.spinner("gui_scroll_speed");
                    ctx.canvas.settings.canvas_scroll_speed =
                        self.panel.spinner("canvas_scroll_speed");
                    ctx.canvas.settings.min_zoom_for_detail = self.panel.dropdown_value("min zoom");
                    // Copy the settings into the Options struct, so they're saved.
                    opts.canvas_settings = ctx.canvas.settings.clone();

                    let style = self.panel.dropdown_value("Traffic signal rendering");
                    if opts.traffic_signal_style != style {
                        opts.traffic_signal_style = style;
                        println!("Rerendering traffic signals...");
                        for i in &mut app.mut_draw_map().intersections {
                            *i.draw_traffic_signal.borrow_mut() = None;
                        }
                    }

                    let camera_angle = self.panel.dropdown_value("Camera angle");
                    if opts.camera_angle != camera_angle {
                        opts.camera_angle = camera_angle;
                        ctx.loading_screen("rerendering buildings", |ctx, timer| {
                            let mut all_buildings = GeomBatch::new();
                            let mut all_building_outlines = GeomBatch::new();
                            timer
                                .start_iter("rendering buildings", app.map().all_buildings().len());
                            for b in app.map().all_buildings() {
                                timer.next();
                                DrawBuilding::new(
                                    ctx,
                                    b,
                                    app.map(),
                                    app.cs(),
                                    &opts,
                                    &mut all_buildings,
                                    &mut all_building_outlines,
                                );
                            }
                            for r in &mut app.mut_draw_map().roads {
                                r.clear_rendering();
                            }

                            timer.start("upload geometry");
                            app.mut_draw_map().draw_all_buildings = all_buildings.upload(ctx);
                            app.mut_draw_map().draw_all_building_outlines =
                                all_building_outlines.upload(ctx);
                            timer.stop("upload geometry");
                        });
                    }

                    if app.change_color_scheme(ctx, self.panel.dropdown_value("Color scheme")) {
                        // change_color_scheme doesn't modify our local copy of Options!
                        opts.color_scheme = app.opts().color_scheme;
                        // If the player picks a different scheme, don't undo it later.
                        opts.toggle_day_night_colors = false;
                    }

                    opts.units.metric = self.panel.is_checked("metric / imperial units");

                    let language = self.panel.dropdown_value("language");
                    if language != opts.language {
                        opts.language = language;
                        for r in &mut app.mut_draw_map().roads {
                            r.clear_rendering();
                        }
                    }

                    // Be careful -- there are some options not exposed by this panel, but per app.
                    let show_building_driveways = opts.show_building_driveways;
                    opts.show_building_driveways = true;
                    let show_building_outlines = opts.show_building_outlines;
                    opts.show_building_outlines = true;
                    abstio::write_json(abstio::path_player("settings.json"), &opts);
                    opts.show_building_driveways = show_building_driveways;
                    opts.show_building_outlines = show_building_outlines;
                    *app.mut_opts() = opts;

                    return widgetry::Transition::Pop;
                }
                _ => unreachable!(),
            }
        }

        widgetry::Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &A) {
        grey_out_map(g, app);
        self.panel.draw(g);
    }
}
