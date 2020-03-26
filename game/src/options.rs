use crate::app::App;
use crate::colors;
use crate::game::{State, Transition};
use ezgui::{
    hotkey, Btn, Choice, Composite, EventCtx, GfxCtx, Key, Line, Outcome, TextExt, Widget,
};

// TODO SimOptions stuff too
#[derive(Clone)]
pub struct Options {
    pub traffic_signal_style: TrafficSignalStyle,
    pub color_scheme: Option<String>,
    pub dev: bool,
}

impl Options {
    pub fn default() -> Options {
        Options {
            traffic_signal_style: TrafficSignalStyle::GroupArrows,
            color_scheme: None,
            dev: false,
        }
    }
}

#[derive(Clone, PartialEq)]
pub enum TrafficSignalStyle {
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
                    Widget::checkbox(ctx, "Enable developer mode", None, app.opts.dev).margin(5),
                    Widget::checkbox(
                        ctx,
                        "Invert direction of vertical scrolling",
                        None,
                        ctx.canvas.invert_scroll,
                    )
                    .margin(5),
                    Widget::checkbox(
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
                            app.opts.color_scheme.clone(),
                            vec![
                                Choice::new("default", None),
                                Choice::new(
                                    "overridden colors",
                                    Some("../data/system/override_colors.json".to_string()),
                                ),
                                Choice::new(
                                    "night mode",
                                    Some("../data/system/night_colors.json".to_string()),
                                ),
                            ],
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
                    Btn::text_bg2("Apply")
                        .build_def(ctx, hotkey(Key::Enter))
                        .margin(5)
                        .centered_horiz(),
                ])
                .bg(colors::PANEL_BG),
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
