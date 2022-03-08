use widgetry::{
    lctrl, EventCtx, HorizontalAlignment, Key, Line, Outcome, Panel, PanelBuilder,
    VerticalAlignment, Widget,
};

use crate::{App, BrowseNeighborhoods, Transition};

pub fn app_top_panel(ctx: &mut EventCtx, app: &App) -> Panel {
    Panel::new_builder(Widget::row(vec![
        map_gui::tools::home_btn(ctx),
        Line("Low traffic neighborhoods")
            .small_heading()
            .into_widget(ctx)
            .centered_vert(),
        map_gui::tools::change_map_btn(ctx, app),
        ctx.style()
            .btn_plain
            .icon("system/assets/tools/search.svg")
            .hotkey(lctrl(Key::F))
            .build_widget(ctx, "search")
            .align_right(),
    ]))
    .aligned(HorizontalAlignment::Left, VerticalAlignment::Top)
    .exact_width_percent(1.0)
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

pub fn left_panel_builder(contents: Widget) -> PanelBuilder {
    Panel::new_builder(contents)
        // TODO Vertical alignment below top panel is brittle
        .aligned(
            HorizontalAlignment::Percent(0.0),
            VerticalAlignment::Percent(0.1),
        )
        .exact_height_percent(0.9)
}
