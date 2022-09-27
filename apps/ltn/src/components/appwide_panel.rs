use geom::CornerRadii;
use widgetry::tools::ChooseSomething;
use widgetry::tools::PopupMsg;
use widgetry::{
    lctrl, Choice, Color, CornerRounding, EventCtx, GfxCtx, HorizontalAlignment, Key, Line,
    Outcome, Panel, PanelDims, Toggle, VerticalAlignment, Widget,
};

use crate::components::Mode;
use crate::{App, PickArea, Transition};

/// Both the top panel and the collapsible left sidebar.
pub struct AppwidePanel {
    pub top_panel: Panel,
    pub left_panel: Panel,
}

impl AppwidePanel {
    pub fn new(ctx: &mut EventCtx, app: &App, mode: Mode) -> Self {
        let top_panel = make_top_panel(ctx, app, mode);
        let left_panel = make_left_panel(ctx, app, &top_panel, mode);
        Self {
            top_panel,
            left_panel,
        }
    }

    pub fn event<F: Fn() -> Vec<&'static str>>(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        preserve_state: &crate::save::PreserveState,
        help: F,
    ) -> Option<Transition> {
        if let Outcome::Clicked(x) = self.top_panel.event(ctx) {
            return match x.as_ref() {
                "Home" => {
                    if app.per_map.consultation.is_none() {
                        Some(Transition::Clear(vec![
                            map_gui::tools::TitleScreen::new_state(
                                ctx,
                                app,
                                map_gui::tools::Executable::LTN,
                                Box::new(|ctx, app, _| PickArea::new_state(ctx, app)),
                            ),
                        ]))
                    } else {
                        Some(Transition::Push(super::about::About::new_state(ctx)))
                    }
                }
                "change map" => Some(Transition::Push(map_gui::tools::CityPicker::new_state(
                    ctx,
                    app,
                    Box::new(|ctx, app| Transition::Replace(PickArea::new_state(ctx, app))),
                ))),
                "search" => Some(Transition::Push(
                    map_gui::tools::Navigator::new_state_with_target_zoom(ctx, app, 4.0),
                )),
                "help" => Some(Transition::Push(PopupMsg::new_state(ctx, "Help", help()))),
                "about this tool" => Some(Transition::Push(super::about::About::new_state(ctx))),
                "Pick area" => Some(Transition::Replace(PickArea::new_state(ctx, app))),
                "Design LTN" => Some(Transition::Replace(
                    crate::design_ltn::DesignLTN::new_state(
                        ctx,
                        app,
                        app.per_map.current_neighbourhood.unwrap(),
                    ),
                )),
                "Plan route" => Some(Transition::Replace(
                    crate::route_planner::RoutePlanner::new_state(ctx, app),
                )),
                "Predict impact" => Some(launch_impact(ctx, app)),
                _ => unreachable!(),
            };
        }

        match self.left_panel.event(ctx) {
            Outcome::Clicked(x) => {
                crate::save::AltProposals::handle_action(ctx, app, preserve_state, &x)
            }
            Outcome::Changed(_) => {
                app.session.manage_proposals = self.left_panel.is_checked("Manage proposals");
                Some(Transition::Recreate)
            }
            _ => None,
        }
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        self.top_panel.draw(g);
        self.left_panel.draw(g);
    }
}

fn launch_impact(ctx: &mut EventCtx, app: &mut App) -> Transition {
    if &app.per_map.impact.map == app.per_map.map.get_name()
        && app.per_map.impact.change_key == app.per_map.edits.get_change_key()
    {
        return Transition::Replace(crate::impact::ShowResults::new_state(ctx, app));
    }

    Transition::Push(ChooseSomething::new_state(ctx,
        "Impact prediction is experimental. You have to interpret the results carefully. The app may also freeze while calculating this.",
        Choice::strings(vec!["Never mind", "I understand the warnings. Predict impact!"]),
        Box::new(|choice, ctx, app| {
            if choice == "Never mind" {
                Transition::Pop
            } else {
                Transition::Multi(vec![
                                  Transition::Pop,
                                  Transition::Replace(crate::impact::ShowResults::new_state(ctx, app)),
                ])
            }
        })))
}

fn make_top_panel(ctx: &mut EventCtx, app: &App, mode: Mode) -> Panel {
    let consultation = app.per_map.consultation.is_some();

    // While we're adjusting a boundary, it's weird to navigate away without explicitly confirming
    // or reverting the edits. Just remove the nav bar entirely.
    let navbar = if mode != Mode::SelectBoundary {
        Widget::row(vec![
            ctx.style()
                .btn_outline
                .text("Pick area")
                .disabled(mode == Mode::PickArea || app.per_map.consultation.is_some())
                .maybe_disabled_tooltip(if mode == Mode::PickArea {
                    None
                } else {
                    Some("This consultation is only about the current area")
                })
                .build_def(ctx),
            ctx.style()
                .btn_outline
                .text("Design LTN")
                .disabled(
                    mode == Mode::ModifyNeighbourhood
                        || app.per_map.current_neighbourhood.is_none(),
                )
                .maybe_disabled_tooltip(if mode == Mode::ModifyNeighbourhood {
                    None
                } else {
                    Some("Pick an area first")
                })
                .build_def(ctx),
            ctx.style()
                .btn_outline
                .text("Plan route")
                .hotkey(Key::R)
                .disabled(mode == Mode::RoutePlanner)
                .build_def(ctx),
            ctx.style()
                .btn_outline
                .text("Predict impact")
                .disabled(mode == Mode::Impact || app.per_map.consultation.is_some())
                .maybe_disabled_tooltip(if mode == Mode::Impact {
                    None
                } else {
                    Some("Not supported here yet")
                })
                .build_def(ctx),
        ])
        .centered_vert()
        .padding(16)
        .outline((5.0, Color::BLACK))
    } else {
        Widget::nothing()
    };
    let col = vec![Widget::row(vec![
        map_gui::tools::home_btn(ctx),
        Line(if consultation {
            "East Bristol Liveable Neighbourhood"
        } else {
            "Low traffic neighbourhoods"
        })
        .small_heading()
        .into_widget(ctx)
        .centered_vert(),
        ctx.style()
            .btn_plain
            .icon("system/assets/tools/info.svg")
            .build_widget(ctx, "about this tool")
            .centered_vert()
            .hide(consultation),
        map_gui::tools::change_map_btn(ctx, app)
            .centered_vert()
            .hide(consultation),
        navbar,
        Widget::row(vec![
            ctx.style()
                .btn_plain
                .icon("system/assets/tools/search.svg")
                .hotkey(lctrl(Key::F))
                .build_widget(ctx, "search")
                .centered_vert(),
            ctx.style()
                .btn_plain
                .icon("system/assets/tools/help.svg")
                .build_widget(ctx, "help")
                .centered_vert(),
        ])
        .align_right(),
    ])];

    Panel::new_builder(
        Widget::col(col).corner_rounding(CornerRounding::CornerRadii(CornerRadii {
            top_left: 0.0,
            bottom_left: 0.0,
            bottom_right: 0.0,
            top_right: 0.0,
        })),
    )
    .aligned(HorizontalAlignment::Left, VerticalAlignment::Top)
    .dims_width(PanelDims::ExactPercent(1.0))
    .build(ctx)
}

fn make_left_panel(ctx: &mut EventCtx, app: &App, top_panel: &Panel, mode: Mode) -> Panel {
    let mut col = vec![Toggle::checkbox(
        ctx,
        "Manage proposals",
        None,
        app.session.manage_proposals,
    )];

    // Switching proposals in some modes is too complex to implement, so don't allow it
    if app.session.manage_proposals && mode != Mode::Impact && mode != Mode::SelectBoundary {
        col.push(app.per_map.alt_proposals.to_widget(ctx, app));
    }

    let top_height = top_panel.panel_dims().height;
    Panel::new_builder(
        Widget::col(col).corner_rounding(CornerRounding::CornerRadii(CornerRadii {
            top_left: 0.0,
            bottom_left: 0.0,
            bottom_right: 0.0,
            top_right: 0.0,
        })),
    )
    .aligned(
        HorizontalAlignment::Left,
        VerticalAlignment::Below(top_height),
    )
    .dims_height(PanelDims::ExactPixels(
        ctx.canvas.window_height - top_height,
    ))
    .build(ctx)
}
