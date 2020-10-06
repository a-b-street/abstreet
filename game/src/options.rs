use geom::Duration;
use widgetry::{
    Btn, Checkbox, Choice, EventCtx, GeomBatch, GfxCtx, Key, Line, Outcome, Panel, Spinner,
    TextExt, Widget,
};

use crate::app::App;
use crate::colors::ColorSchemeChoice;
use crate::game::{State, Transition};
use crate::render::DrawBuilding;

// TODO SimOptions stuff too
#[derive(Clone)]
pub struct Options {
    pub dev: bool,
    pub debug_all_agents: bool,

    pub label_roads: bool,
    pub traffic_signal_style: TrafficSignalStyle,
    pub color_scheme: ColorSchemeChoice,
    pub min_zoom_for_detail: f64,
    pub large_unzoomed_agents: bool,
    pub camera_angle: CameraAngle,

    pub time_increment: Duration,
    pub dont_draw_time_warp: bool,
    pub jump_to_delay: Duration,

    pub language: Option<String>,
}

impl Options {
    pub fn default() -> Options {
        Options {
            dev: false,
            debug_all_agents: false,

            label_roads: true,
            traffic_signal_style: TrafficSignalStyle::BAP,
            color_scheme: ColorSchemeChoice::Standard,
            min_zoom_for_detail: 4.0,
            large_unzoomed_agents: false,
            camera_angle: CameraAngle::TopDown,

            time_increment: Duration::minutes(10),
            dont_draw_time_warp: false,
            jump_to_delay: Duration::minutes(5),

            language: None,
        }
    }
}

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
}

pub struct OptionsPanel {
    panel: Panel,
}

impl OptionsPanel {
    pub fn new(ctx: &mut EventCtx, app: &App) -> Box<dyn State> {
        Box::new(OptionsPanel {
            panel: Panel::new(Widget::col(vec![
                Widget::custom_row(vec![
                    Line("Settings").small_heading().draw(ctx),
                    Btn::plaintext("X")
                        .build(ctx, "close", Key::Escape)
                        .align_right(),
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
                .bg(app.cs.section_bg)
                .padding(8),
                "Appearance".draw_text(ctx),
                Widget::col(vec![
                    Checkbox::checkbox(ctx, "Draw road names", None, app.opts.label_roads),
                    Widget::row(vec![
                        "Traffic signal rendering:".draw_text(ctx),
                        Widget::dropdown(
                            ctx,
                            "Traffic signal rendering",
                            app.opts.traffic_signal_style.clone(),
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
                            app.opts.camera_angle.clone(),
                            vec![
                                Choice::new("Top-down", CameraAngle::TopDown),
                                Choice::new("Isometric (northeast)", CameraAngle::IsometricNE),
                                Choice::new("Isometric (northwest)", CameraAngle::IsometricNW),
                                Choice::new("Isometric (southeast)", CameraAngle::IsometricSE),
                                Choice::new("Isometric (southwest)", CameraAngle::IsometricSW),
                            ],
                        ),
                    ]),
                    Widget::row(vec![
                        "Color scheme:".draw_text(ctx),
                        Widget::dropdown(
                            ctx,
                            "Color scheme",
                            app.opts.color_scheme,
                            ColorSchemeChoice::choices(),
                        ),
                    ]),
                    Widget::row(vec![
                        "Camera zoom to switch to unzoomed view".draw_text(ctx),
                        Widget::dropdown(
                            ctx,
                            "min zoom",
                            app.opts.min_zoom_for_detail,
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
                    Checkbox::checkbox(
                        ctx,
                        "Draw enlarged unzoomed agents",
                        None,
                        app.opts.large_unzoomed_agents,
                    ),
                    Widget::row(vec![
                        "Language".draw_text(ctx),
                        Widget::dropdown(ctx, "language", app.opts.language.clone(), {
                            let mut choices = Vec::new();
                            choices.push(Choice::new("Map native language", None));
                            for lang in app.primary.map.get_languages() {
                                choices.push(Choice::new(lang, Some(lang.to_string())));
                            }
                            choices
                        }),
                    ]),
                ])
                .bg(app.cs.section_bg)
                .padding(8),
                "Debug".draw_text(ctx),
                Widget::col(vec![
                    Checkbox::checkbox(ctx, "Enable developer mode", None, app.opts.dev),
                    Checkbox::checkbox(
                        ctx,
                        "Draw all agents to debug geometry (Slow!)",
                        None,
                        app.opts.debug_all_agents,
                    ),
                ])
                .bg(app.cs.section_bg)
                .padding(8),
                Btn::text_bg2("Apply")
                    .build_def(ctx, Key::Enter)
                    .centered_horiz(),
            ]))
            .build(ctx),
        })
    }
}

impl State for OptionsPanel {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                "Apply" => {
                    app.opts.dev = self.panel.is_checked("Enable developer mode");
                    app.opts.debug_all_agents = self
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

                    app.opts.label_roads = self.panel.is_checked("Draw road names");
                    let style = self.panel.dropdown_value("Traffic signal rendering");
                    if app.opts.traffic_signal_style != style {
                        app.opts.traffic_signal_style = style;
                        println!("Rerendering traffic signals...");
                        for i in &mut app.primary.draw_map.intersections {
                            *i.draw_traffic_signal.borrow_mut() = None;
                        }
                    }

                    let camera_angle = self.panel.dropdown_value("Camera angle");
                    if app.opts.camera_angle != camera_angle {
                        app.opts.camera_angle = camera_angle;
                        ctx.loading_screen("rerendering buildings", |ctx, timer| {
                            let mut all_buildings = GeomBatch::new();
                            let mut all_building_paths = GeomBatch::new();
                            let mut all_building_outlines = GeomBatch::new();
                            timer.start_iter(
                                "rendering buildings",
                                app.primary.map.all_buildings().len(),
                            );
                            for b in app.primary.map.all_buildings() {
                                timer.next();
                                DrawBuilding::new(
                                    ctx,
                                    b,
                                    &app.primary.map,
                                    &app.cs,
                                    &app.opts,
                                    &mut all_buildings,
                                    &mut all_building_paths,
                                    &mut all_building_outlines,
                                );
                            }
                            timer.start("upload geometry");
                            app.primary.draw_map.draw_all_buildings = all_buildings.upload(ctx);
                            app.primary.draw_map.draw_all_building_paths =
                                all_building_paths.upload(ctx);
                            app.primary.draw_map.draw_all_building_outlines =
                                all_building_outlines.upload(ctx);
                            timer.stop("upload geometry");
                        });
                    }

                    let scheme = self.panel.dropdown_value("Color scheme");
                    if app.opts.color_scheme != scheme {
                        app.opts.color_scheme = scheme;
                        app.switch_map(ctx, app.primary.current_flags.sim_flags.load.clone());
                    }

                    app.opts.min_zoom_for_detail = self.panel.dropdown_value("min zoom");
                    app.opts.large_unzoomed_agents =
                        self.panel.is_checked("Draw enlarged unzoomed agents");

                    let language = self.panel.dropdown_value("language");
                    if language != app.opts.language {
                        app.opts.language = language;
                        for r in &mut app.primary.draw_map.roads {
                            r.clear_rendering();
                        }
                    }

                    return Transition::Pop;
                }
                _ => unreachable!(),
            },
            _ => {}
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        State::grey_out_map(g, app);
        self.panel.draw(g);
    }
}
