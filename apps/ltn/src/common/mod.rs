use geom::CornerRadii;
use map_gui::tools::grey_out_map;
use widgetry::tools::{open_browser, PopupMsg};
use widgetry::{
    lctrl, CornerRounding, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Panel,
    PanelBuilder, PanelDims, SimpleState, State, TextExt, VerticalAlignment, Widget,
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
            ctx.style()
                .btn_plain
                .icon("system/assets/tools/info.svg")
                .build_widget(ctx, "about this tool")
                .centered_vert(),
            map_gui::tools::change_map_btn(ctx, app).centered_vert(),
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

pub fn handle_top_panel<F: Fn() -> Vec<&'static str>>(
    ctx: &mut EventCtx,
    app: &App,
    panel: &mut Panel,
    help: F,
) -> Option<Transition> {
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
            "help" => Some(Transition::Push(PopupMsg::new_state(ctx, "Help", help()))),
            "about this tool" => Some(Transition::Push(About::new_state(ctx))),
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

struct About;

impl About {
    fn new_state(ctx: &mut EventCtx) -> Box<dyn State<App>> {
        let panel = Panel::new_builder(Widget::col(vec![
            Widget::row(vec![
                Line("About the LTN tool").small_heading().into_widget(ctx),
                ctx.style().btn_close_widget(ctx),
            ]),
            "Created by Dustin Carlino & Cindy Huang".text_widget(ctx),
            "Data from OpenStreetMap".text_widget(ctx),
            "See below for full credits and more info".text_widget(ctx),
            ctx.style()
                .btn_outline
                .text("ltn.abstreet.org")
                .build_def(ctx),
        ]))
        .build(ctx);
        <dyn SimpleState<_>>::new_state(panel, Box::new(About))
    }
}

impl SimpleState<App> for About {
    fn on_click(&mut self, _: &mut EventCtx, _: &mut App, x: &str, _: &mut Panel) -> Transition {
        if x == "close" {
            return Transition::Pop;
        } else if x == "ltn.abstreet.org" {
            open_browser("http://ltn.abstreet.org");
        }
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        grey_out_map(g, app);
    }
}
