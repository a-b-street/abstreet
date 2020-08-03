use crate::app::App;
use crate::colors::ColorSchemeChoice;
use crate::game::{State, Transition};
use ezgui::{
    hotkey, Btn, Checkbox, Choice, Composite, EventCtx, GfxCtx, Key, Line, Outcome, Spinner,
    TextExt, Widget,
};
use geom::Duration;

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

    pub time_increment: Duration,
    pub resume_after_edit: bool,
    pub dont_draw_time_warp: bool,
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

            time_increment: Duration::minutes(10),
            resume_after_edit: true,
            dont_draw_time_warp: false,
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum TrafficSignalStyle {
    BAP,
    GroupArrows,
    Sidewalks,
    Icons,
    IndividualTurnArrows,
}

pub struct OptionsPanel {
    composite: Composite,
}

impl OptionsPanel {
    pub fn new(ctx: &mut EventCtx, app: &App) -> OptionsPanel {
        OptionsPanel {
            composite: Composite::new(Widget::col(vec![
                Widget::custom_row(vec![
                    Line("Settings").small_heading().draw(ctx),
                    Btn::plaintext("X")
                        .build(ctx, "close", hotkey(Key::Escape))
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
                                Choice::new(
                                    "Brian's variation of arrows showing the protected and \
                                     permitted movements",
                                    TrafficSignalStyle::BAP,
                                ),
                                Choice::new(
                                    "arrows showing the protected and permitted movements",
                                    TrafficSignalStyle::GroupArrows,
                                ),
                                Choice::new(
                                    "arrows showing the protected and permitted movements, with \
                                     sidewalks",
                                    TrafficSignalStyle::Sidewalks,
                                ),
                                Choice::new(
                                    "icons for movements (like the editor UI)",
                                    TrafficSignalStyle::Icons,
                                ),
                                Choice::new(
                                    "arrows showing individual turns (to debug)",
                                    TrafficSignalStyle::IndividualTurnArrows,
                                ),
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
                        format!(
                            "Scale factor for text / UI elements (your monitor is {}):",
                            ctx.monitor_scale_factor()
                        )
                        .draw_text(ctx),
                        Widget::dropdown(ctx, "Scale factor", ctx.get_scale_factor(), {
                            let mut choices = vec![
                                Choice::new("0.5", 0.5),
                                Choice::new("1.0", 1.0),
                                Choice::new("1.5", 1.5),
                                Choice::new("2.0", 2.0),
                            ];
                            let native = ctx.monitor_scale_factor();
                            if !choices.iter().any(|c| c.data == native) {
                                choices.push(Choice::new(native.to_string(), native));
                            }
                            choices
                        }),
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
                    .build_def(ctx, hotkey(Key::Enter))
                    .centered_horiz(),
            ]))
            .build(ctx),
        }
    }
}

impl State for OptionsPanel {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.composite.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                "Apply" => {
                    app.opts.dev = self.composite.is_checked("Enable developer mode");
                    app.opts.debug_all_agents = self
                        .composite
                        .is_checked("Draw all agents to debug geometry (Slow!)");

                    ctx.canvas.invert_scroll = self
                        .composite
                        .is_checked("Invert direction of vertical scrolling");
                    ctx.canvas.touchpad_to_move = self
                        .composite
                        .is_checked("Use touchpad to pan and hold Control to zoom");
                    ctx.canvas.keys_to_pan = self
                        .composite
                        .is_checked("Use arrow keys to pan and Q/W to zoom");
                    ctx.canvas.edge_auto_panning = self.composite.is_checked("autopan");
                    ctx.canvas.gui_scroll_speed =
                        self.composite.spinner("gui_scroll_speed") as usize;

                    app.opts.label_roads = self.composite.is_checked("Draw road names");
                    let style = self.composite.dropdown_value("Traffic signal rendering");
                    if app.opts.traffic_signal_style != style {
                        app.opts.traffic_signal_style = style;
                        println!("Rerendering traffic signals...");
                        for i in app.primary.draw_map.intersections.iter_mut() {
                            *i.draw_traffic_signal.borrow_mut() = None;
                        }
                    }

                    let scheme = self.composite.dropdown_value("Color scheme");
                    if app.opts.color_scheme != scheme {
                        app.opts.color_scheme = scheme;
                        app.switch_map(ctx, app.primary.current_flags.sim_flags.load.clone());
                    }

                    let factor = self.composite.dropdown_value("Scale factor");
                    if ctx.get_scale_factor() != factor {
                        ctx.set_scale_factor(factor);
                    }

                    app.opts.min_zoom_for_detail = self.composite.dropdown_value("min zoom");
                    app.opts.large_unzoomed_agents =
                        self.composite.is_checked("Draw enlarged unzoomed agents");

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
        self.composite.draw(g);
    }
}
