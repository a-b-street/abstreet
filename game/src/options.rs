use crate::app::App;
use crate::colors::ColorSchemeChoice;
use crate::game::{State, Transition};
use ezgui::{
    hotkey, Btn, Checkbox, Choice, Composite, EventCtx, GfxCtx, Key, Line, Outcome, TextExt, Widget,
};
use geom::Duration;

// TODO SimOptions stuff too
#[derive(Clone)]
pub struct Options {
    pub traffic_signal_style: TrafficSignalStyle,
    pub color_scheme: ColorSchemeChoice,
    pub dev: bool,
    pub time_increment: Duration,
    pub min_zoom_for_detail: f64,
    pub large_unzoomed_agents: bool,
}

impl Options {
    pub fn default() -> Options {
        Options {
            traffic_signal_style: TrafficSignalStyle::BAP,
            color_scheme: ColorSchemeChoice::Standard,
            dev: false,
            time_increment: Duration::minutes(10),
            min_zoom_for_detail: 4.0,
            large_unzoomed_agents: false,
        }
    }
}

#[derive(Clone, PartialEq)]
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
            composite: Composite::new(
                Widget::col(vec![
                    Widget::row(vec![
                        Line("Settings").small_heading().draw(ctx),
                        Btn::text_fg("X")
                            .build_def(ctx, hotkey(Key::Escape))
                            .align_right(),
                    ]),
                    Checkbox::text(ctx, "Enable developer mode", None, app.opts.dev).margin(5),
                    Checkbox::text(
                        ctx,
                        "Invert direction of vertical scrolling",
                        None,
                        ctx.canvas.invert_scroll,
                    )
                    .margin(5),
                    Checkbox::text(
                        ctx,
                        "Enable panning map when cursor is at edge of screen",
                        None,
                        ctx.canvas.edge_auto_panning,
                    )
                    .named("disable pan"),
                    Checkbox::text(
                        ctx,
                        "Use touchpad to pan and hold Control to zoom",
                        None,
                        ctx.canvas.touchpad_to_move,
                    )
                    .margin(5),
                    Widget::row(vec![
                        "Traffic signal rendering:".draw_text(ctx).margin(5),
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
                        "Color scheme:".draw_text(ctx).margin(5),
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
                        .draw_text(ctx)
                        .margin(5),
                        Widget::dropdown(
                            ctx,
                            "Scale factor",
                            ctx.get_scale_factor(),
                            vec![
                                Choice::new("0.5", 0.5),
                                Choice::new("1.0", 1.0),
                                Choice::new("1.5", 1.5),
                                Choice::new("2.0", 2.0),
                            ],
                        ),
                    ]),
                    Widget::row(vec![
                        "Camera zoom to switch to unzoomed view"
                            .draw_text(ctx)
                            .margin(5),
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
                    Checkbox::text(
                        ctx,
                        "Draw enlarged unzoomed agents",
                        None,
                        app.opts.large_unzoomed_agents,
                    )
                    .margin(5),
                    Btn::text_bg2("Apply")
                        .build_def(ctx, hotkey(Key::Enter))
                        .margin(5)
                        .centered_horiz(),
                ])
                .padding(10)
                .bg(app.cs.panel_bg),
            )
            .build(ctx),
        }
    }
}

impl State for OptionsPanel {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "X" => {
                    return Transition::Pop;
                }
                "Apply" => {
                    ctx.canvas.invert_scroll = self
                        .composite
                        .is_checked("Invert direction of vertical scrolling");
                    ctx.canvas.touchpad_to_move = self
                        .composite
                        .is_checked("Use touchpad to pan and hold Control to zoom");
                    ctx.canvas.edge_auto_panning = self.composite.is_checked("disable pan");
                    app.opts.dev = self.composite.is_checked("Enable developer mode");

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
            None => {}
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.composite.draw(g);
    }
}
