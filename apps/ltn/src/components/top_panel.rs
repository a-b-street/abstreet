use geom::CornerRadii;
use widgetry::tools::PopupMsg;
use widgetry::{
    lctrl, CornerRounding, EventCtx, HorizontalAlignment, Key, Line, Outcome, Panel, PanelDims,
    VerticalAlignment, Widget,
};

use crate::{App, BrowseNeighbourhoods, Transition};

pub struct TopPanel;

impl TopPanel {
    pub fn panel(ctx: &mut EventCtx, app: &App) -> Panel {
        let consultation = app.session.consultation.is_some();

        Panel::new_builder(
            Widget::row(vec![
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
                Widget::row(vec![
                    ctx.style()
                        .btn_plain
                        .text("Export to GeoJSON")
                        .build_def(ctx)
                        .centered_vert()
                        .hide(consultation),
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
            ])
            .corner_rounding(CornerRounding::CornerRadii(CornerRadii {
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

    pub fn event<F: Fn() -> Vec<&'static str>>(
        ctx: &mut EventCtx,
        app: &App,
        panel: &mut Panel,
        help: F,
    ) -> Option<Transition> {
        if let Outcome::Clicked(x) = panel.event(ctx) {
            match x.as_ref() {
                "Home" => {
                    if app.session.consultation.is_none() {
                        Some(Transition::Clear(vec![
                            map_gui::tools::TitleScreen::new_state(
                                ctx,
                                app,
                                map_gui::tools::Executable::LTN,
                                Box::new(|ctx, app, _| BrowseNeighbourhoods::new_state(ctx, app)),
                            ),
                        ]))
                    } else {
                        Some(Transition::Push(super::about::About::new_state(ctx)))
                    }
                }
                "change map" => Some(Transition::Push(map_gui::tools::CityPicker::new_state(
                    ctx,
                    app,
                    Box::new(|ctx, app| {
                        Transition::Replace(BrowseNeighbourhoods::new_state(ctx, app))
                    }),
                ))),
                "search" => Some(Transition::Push(
                    map_gui::tools::Navigator::new_state_with_target_zoom(ctx, app, 4.0),
                )),
                "help" => Some(Transition::Push(PopupMsg::new_state(ctx, "Help", help()))),
                "about this tool" => Some(Transition::Push(super::about::About::new_state(ctx))),
                "Export to GeoJSON" => {
                    let result = crate::export::write_geojson_file(ctx, app);
                    Some(Transition::Push(match result {
                        Ok(path) => PopupMsg::new_state(
                            ctx,
                            "LTNs exported",
                            vec![format!("Data exported to {}", path)],
                        ),
                        Err(err) => {
                            PopupMsg::new_state(ctx, "Export failed", vec![err.to_string()])
                        }
                    }))
                }
                _ => unreachable!(),
            }
        } else {
            None
        }
    }
}
