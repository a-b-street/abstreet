use geom::CornerRadii;
use widgetry::{
    lctrl, CornerRounding, EventCtx, HorizontalAlignment, Key, Line, Outcome, Panel, PanelBuilder,
    PanelDims, VerticalAlignment, Widget,
};

use crate::{App, BrowseNeighborhoods, Transition};

pub fn app_top_panel(ctx: &mut EventCtx, app: &App) -> Panel {
    Panel::new_builder(
        Widget::row(vec![
            map_gui::tools::home_btn(ctx),
            Line("Low traffic neighborhoods")
                .small_heading()
                .into_widget(ctx)
                .centered_vert(),
            map_gui::tools::change_map_btn(ctx, app).centered_vert(),
            ctx.style()
                .btn_plain
                .icon("system/assets/tools/search.svg")
                .hotkey(lctrl(Key::F))
                .build_widget(ctx, "search")
                .centered_vert(),
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

pub fn handle_top_panel(ctx: &mut EventCtx, app: &App, panel: &mut Panel) -> Option<Transition> {
    if let Outcome::Clicked(x) = panel.event(ctx) {
        match x.as_ref() {
            "Home" => Some(Transition::Clear(vec![
                map_gui::tools::TitleScreen::new_state(
                    ctx,
                    app,
                    map_gui::tools::Executable::LTN,
                    Box::new(|ctx, app, _| BrowseNeighborhoods::new_state(ctx, app)),
                ),
            ])),
            "change map" => Some(Transition::Push(map_gui::tools::CityPicker::new_state(
                ctx,
                app,
                Box::new(|ctx, app| Transition::Replace(BrowseNeighborhoods::new_state(ctx, app))),
            ))),
            "search" => Some(Transition::Push(map_gui::tools::Navigator::new_state(
                ctx, app,
            ))),
            _ => unreachable!(),
        }
    } else {
        None
    }
}

pub fn left_panel_builder(ctx: &EventCtx, top_panel: &Panel, contents: Widget) -> PanelBuilder {
    let top_height = top_panel.panel_dims().height;
    Panel::new_builder(
        contents.corner_rounding(CornerRounding::CornerRadii(CornerRadii {
            top_left: 0.0,
            bottom_left: 0.0,
            bottom_right: 0.0,
            top_right: 0.0,
        })),
    )
    .aligned(
        HorizontalAlignment::Percent(0.0),
        VerticalAlignment::Below(top_height),
    )
    .dims_height(PanelDims::ExactPixels(
        ctx.canvas.window_height - top_height,
    ))
}
