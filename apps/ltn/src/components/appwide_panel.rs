use widgetry::tools::ChooseSomething;
use widgetry::tools::PopupMsg;
use widgetry::{
    lctrl, Choice, CornerRounding, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Outcome,
    Panel, PanelDims, VerticalAlignment, Widget,
};

use crate::components::Mode;
use crate::{pages, App, Transition};

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
                                Box::new(|ctx, app, _| pages::PickArea::new_state(ctx, app)),
                            ),
                        ]))
                    } else {
                        Some(Transition::Push(pages::About::new_state(ctx)))
                    }
                }
                "change map" => Some(Transition::Push(map_gui::tools::CityPicker::new_state(
                    ctx,
                    app,
                    Box::new(|ctx, app| Transition::Replace(pages::PickArea::new_state(ctx, app))),
                ))),
                "search" => Some(Transition::Push(
                    map_gui::tools::Navigator::new_state_with_target_zoom(ctx, app, 4.0),
                )),
                "help" => Some(Transition::Push(PopupMsg::new_state(ctx, "Help", help()))),
                "about this tool" => Some(Transition::Push(pages::About::new_state(ctx))),
                "Pick area" => Some(Transition::Replace(pages::PickArea::new_state(ctx, app))),
                "Design LTN" => Some(Transition::Replace(pages::DesignLTN::new_state(
                    ctx,
                    app,
                    app.per_map.current_neighbourhood.unwrap(),
                ))),
                "Plan route" => Some(Transition::Replace(pages::RoutePlanner::new_state(
                    ctx, app,
                ))),
                "Crossings" => Some(Transition::Replace(pages::Crossings::new_state(ctx, app))),
                "Predict impact" => Some(launch_impact(ctx, app)),
                "Cycle network" => Some(Transition::Replace(pages::CycleNetwork::new_state(
                    ctx, app,
                ))),
                _ => unreachable!(),
            };
        }

        if let Outcome::Clicked(x) = self.left_panel.event(ctx) {
            return if x == "show proposals" {
                app.session.manage_proposals = true;
                Some(Transition::Recreate)
            } else if x == "hide proposals" {
                app.session.manage_proposals = false;
                Some(Transition::Recreate)
            } else {
                crate::save::Proposals::handle_action(ctx, app, preserve_state, &x)
            };
        }

        None
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        self.top_panel.draw(g);
        self.left_panel.draw(g);
    }
}

fn launch_impact(ctx: &mut EventCtx, app: &mut App) -> Transition {
    if &app.per_map.impact.map == app.per_map.map.get_name()
        && app.per_map.impact.change_key == app.edits().get_change_key()
    {
        return Transition::Replace(pages::ShowImpactResults::new_state(ctx, app));
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
                                  Transition::Replace(pages::ShowImpactResults::new_state(ctx, app)),
                ])
            }
        })))
}

fn make_top_panel(ctx: &mut EventCtx, app: &App, mode: Mode) -> Panel {
    let consultation = app.per_map.consultation.is_some();

    fn current_mode(ctx: &mut EventCtx, name: &str) -> Widget {
        ctx.style()
            .btn_solid_primary
            .text(name)
            .disabled(true)
            .build_def(ctx)
    }

    // While we're adjusting a boundary, it's weird to navigate away without explicitly confirming
    // or reverting the edits. Just remove the nav bar entirely.
    let navbar = if mode != Mode::SelectBoundary {
        Widget::row(vec![
            if mode == Mode::PickArea {
                current_mode(ctx, "Pick area")
            } else {
                ctx.style()
                    .btn_outline
                    .text("Pick area")
                    .disabled(app.per_map.consultation.is_some())
                    .disabled_tooltip("This consultation is only about the current area")
                    .build_def(ctx)
            },
            if mode == Mode::ModifyNeighbourhood {
                current_mode(ctx, "Design LTN")
            } else {
                ctx.style()
                    .btn_outline
                    .text("Design LTN")
                    .disabled(app.per_map.current_neighbourhood.is_none())
                    .disabled_tooltip("Pick an area first")
                    .build_def(ctx)
            },
            if mode == Mode::RoutePlanner {
                current_mode(ctx, "Plan route")
            } else {
                ctx.style()
                    .btn_outline
                    .text("Plan route")
                    .hotkey(Key::R)
                    .build_def(ctx)
            },
            if mode == Mode::Crossings {
                current_mode(ctx, "Crossings")
            } else {
                ctx.style()
                    .btn_outline
                    .text("Crossings")
                    .hotkey(Key::C)
                    .disabled(app.per_map.consultation.is_some())
                    .disabled_tooltip("Not supported here yet")
                    .build_def(ctx)
            },
            if mode == Mode::Impact {
                current_mode(ctx, "Predict impact")
            } else {
                ctx.style()
                    .btn_outline
                    .text("Predict impact")
                    .disabled(app.per_map.consultation.is_some())
                    .disabled_tooltip("Not supported here yet")
                    .build_def(ctx)
            },
            if mode == Mode::CycleNetwork {
                current_mode(ctx, "Cycle network")
            } else {
                ctx.style().btn_outline.text("Cycle network").build_def(ctx)
            },
        ])
        .centered_vert()
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

    Panel::new_builder(Widget::col(col).corner_rounding(CornerRounding::NoRounding))
        .aligned(HorizontalAlignment::Left, VerticalAlignment::Top)
        .dims_width(PanelDims::ExactPercent(1.0))
        .build(ctx)
}

fn make_left_panel(ctx: &mut EventCtx, app: &App, top_panel: &Panel, mode: Mode) -> Panel {
    let mut col = Vec::new();

    // Switching proposals in some modes is too complex to implement, so don't allow it
    if app.session.manage_proposals && mode != Mode::Impact && mode != Mode::SelectBoundary {
        col.push(
            ctx.style()
                .btn_plain
                .icon("system/assets/tools/collapse_panel.svg")
                .hotkey(Key::P)
                .build_widget(ctx, "hide proposals")
                .align_right(),
        );
        col.push(app.per_map.proposals.to_widget_expanded(ctx, app));
    } else {
        col.push(
            ctx.style()
                .btn_plain
                .icon("system/assets/tools/expand_panel.svg")
                .hotkey(Key::P)
                .build_widget(ctx, "show proposals")
                .align_right(),
        );
        if mode != Mode::Impact && mode != Mode::SelectBoundary {
            col.push(app.per_map.proposals.to_widget_collapsed(ctx));
        }
    }

    let top_height = top_panel.panel_dims().height;
    Panel::new_builder(Widget::col(col).corner_rounding(CornerRounding::NoRounding))
        .aligned(
            HorizontalAlignment::Left,
            VerticalAlignment::Below(top_height),
        )
        .dims_height(PanelDims::ExactPixels(
            ctx.canvas.window_height - top_height,
        ))
        .build(ctx)
}
