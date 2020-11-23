use geom::{Duration, UnitFmt};
use widgetry::{
    Btn, Checkbox, Choice, EventCtx, GeomBatch, GfxCtx, Key, Line, Outcome, Panel, Spinner, State,
    TextExt, Widget,
};

use crate::colors::ColorSchemeChoice;
use crate::helpers::grey_out_map;
use crate::render::DrawBuilding;
use crate::AppLike;

/// Options controlling the UI.
// TODO SimOptions stuff too
#[derive(Clone)]
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
    /// Map elements are drawn differently when unzoomed and zoomed. This specifies the canvas zoom
    /// level where they switch.
    pub min_zoom_for_detail: f64,
    /// Draw buildings in different perspectives
    pub camera_angle: CameraAngle,

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
    pub fn default() -> Options {
        Options {
            dev: false,
            debug_all_agents: false,

            traffic_signal_style: TrafficSignalStyle::BAP,
            color_scheme: ColorSchemeChoice::Standard,
            toggle_day_night_colors: false,
            min_zoom_for_detail: 4.0,
            camera_angle: CameraAngle::TopDown,

            time_increment: Duration::minutes(10),
            dont_draw_time_warp: false,
            jump_to_delay: Duration::minutes(5),

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
#[derive(Clone, PartialEq, Debug)]
pub enum TrafficSignalStyle {
    BAP,
    Yuwen,
    IndividualTurnArrows,
}

#[derive(Clone, PartialEq, Debug)]
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
    pub fn new<A: AppLike>(ctx: &mut EventCtx, app: &A) -> Box<dyn State<A>> {
        Box::new(OptionsPanel {
            panel: Panel::new(Widget::col(vec![
                Widget::custom_row(vec![
                    Line("Settings").small_heading().draw(ctx),
                    Btn::close(ctx),
                ]),
                "Camera controls".draw_text(ctx),
                Widget::col(vec![
                    Checkbox::checkbox(
                        ctx,
                        "Invert direction of vertical scrolling",
                        None,
                        ctx.canvas.invert_scroll,
                    ),
                    Checkbox::checkbox(
                        ctx,
                        "Pan map when cursor is at edge of screen",
                        None,
                        ctx.canvas.edge_auto_panning,
                    )
                    .named("autopan"),
                    Checkbox::checkbox(
                        ctx,
                        "Use touchpad to pan and hold Control to zoom",
                        None,
                        ctx.canvas.touchpad_to_move,
                    ),
                    Checkbox::checkbox(
                        ctx,
                        "Use arrow keys to pan and Q/W to zoom",
                        None,
                        ctx.canvas.keys_to_pan,
                    ),
                    Widget::row(vec![
                        "Scroll speed for menus".draw_text(ctx).centered_vert(),
                        Spinner::new(ctx, (1, 50), ctx.canvas.gui_scroll_speed as isize)
                            .named("gui_scroll_speed"),
                    ]),
                ])
                .bg(app.cs().section_bg)
                .padding(8),
                "Appearance".draw_text(ctx),
                Widget::col(vec![
                    Widget::row(vec![
                        "Traffic signal rendering:".draw_text(ctx),
                        Widget::dropdown(
                            ctx,
                            "Traffic signal rendering",
                            app.opts().traffic_signal_style.clone(),
                            vec![
                                Choice::new("Default (Brian's style)", TrafficSignalStyle::BAP),
                                Choice::new("Yuwen's style", TrafficSignalStyle::Yuwen),
                                Choice::new(
                                    "arrows showing individual turns (to debug)",
                                    TrafficSignalStyle::IndividualTurnArrows,
                                ),
                            ],
                        ),
                    ]),
                    Widget::row(vec![
                        "Camera angle:".draw_text(ctx),
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
                        "Color scheme:".draw_text(ctx),
                        Widget::dropdown(
                            ctx,
                            "Color scheme",
                            app.opts().color_scheme,
                            ColorSchemeChoice::choices(),
                        ),
                    ]),
                    Widget::row(vec![
                        "Camera zoom to switch to unzoomed view".draw_text(ctx),
                        Widget::dropdown(
                            ctx,
                            "min zoom",
                            app.opts().min_zoom_for_detail,
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
                    Widget::row(vec![
                        "Language".draw_text(ctx),
                        Widget::dropdown(ctx, "language", app.opts().language.clone(), {
                            let mut choices = Vec::new();
                            choices.push(Choice::new("Map native language", None));
                            for lang in app.map().get_languages() {
                                choices.push(Choice::new(lang, Some(lang.to_string())));
                            }
                            choices
                        }),
                    ]),
                    Checkbox::toggle(
                        ctx,
                        "metric / imperial units",
                        "metric",
                        "imperial",
                        None,
                        app.opts().units.metric,
                    ),
                ])
                .bg(app.cs().section_bg)
                .padding(8),
                "Debug".draw_text(ctx),
                Widget::col(vec![
                    Checkbox::checkbox(ctx, "Enable developer mode", None, app.opts().dev),
                    Checkbox::checkbox(
                        ctx,
                        "Draw all agents to debug geometry (Slow!)",
                        None,
                        app.opts().debug_all_agents,
                    ),
                ])
                .bg(app.cs().section_bg)
                .padding(8),
                Btn::text_bg2("Apply")
                    .build_def(ctx, Key::Enter)
                    .centered_horiz(),
            ]))
            .build(ctx),
        })
    }
}

impl<A: AppLike> State<A> for OptionsPanel {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut A) -> widgetry::Transition<A> {
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    return widgetry::Transition::Pop;
                }
                "Apply" => {
                    let mut opts = app.opts().clone();
                    opts.dev = self.panel.is_checked("Enable developer mode");
                    opts.debug_all_agents = self
                        .panel
                        .is_checked("Draw all agents to debug geometry (Slow!)");

                    ctx.canvas.invert_scroll = self
                        .panel
                        .is_checked("Invert direction of vertical scrolling");
                    ctx.canvas.touchpad_to_move = self
                        .panel
                        .is_checked("Use touchpad to pan and hold Control to zoom");
                    ctx.canvas.keys_to_pan = self
                        .panel
                        .is_checked("Use arrow keys to pan and Q/W to zoom");
                    ctx.canvas.edge_auto_panning = self.panel.is_checked("autopan");
                    ctx.canvas.gui_scroll_speed = self.panel.spinner("gui_scroll_speed") as usize;

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
                            let mut all_building_paths = GeomBatch::new();
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
                                    &mut all_building_paths,
                                    &mut all_building_outlines,
                                );
                            }
                            timer.start("upload geometry");
                            app.mut_draw_map().draw_all_buildings = all_buildings.upload(ctx);
                            app.mut_draw_map().draw_all_building_paths =
                                all_building_paths.upload(ctx);
                            app.mut_draw_map().draw_all_building_outlines =
                                all_building_outlines.upload(ctx);
                            timer.stop("upload geometry");
                        });
                    }

                    if app.change_color_scheme(ctx, self.panel.dropdown_value("Color scheme")) {
                        // If the player picks a different scheme, don't undo it later.
                        opts.toggle_day_night_colors = false;
                    }

                    opts.min_zoom_for_detail = self.panel.dropdown_value("min zoom");
                    opts.units.metric = self.panel.is_checked("metric / imperial units");

                    let language = self.panel.dropdown_value("language");
                    if language != opts.language {
                        opts.language = language;
                        for r in &mut app.mut_draw_map().roads {
                            r.clear_rendering();
                        }
                    }

                    *app.mut_opts() = opts;

                    return widgetry::Transition::Pop;
                }
                _ => unreachable!(),
            },
            _ => {}
        }

        widgetry::Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &A) {
        grey_out_map(g, app);
        self.panel.draw(g);
    }
}
