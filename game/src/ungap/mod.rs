use geom::{Circle, Distance, Pt2D};
use map_gui::tools::{nice_map_name, CityPicker, PopupMsg};
use widgetry::{
    lctrl, Color, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Panel,
    State, VerticalAlignment, Widget,
};

use crate::app::{App, Transition};

pub struct ExploreMap {
    top_panel: Panel,
    legend: Panel,
}

impl ExploreMap {
    pub fn new_state(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
        Box::new(ExploreMap {
            top_panel: make_top_panel(ctx),
            legend: make_legend(ctx, app),
        })
    }
}

impl State<App> for ExploreMap {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();

        if let Outcome::Clicked(x) = self.top_panel.event(ctx) {
            match x.as_ref() {
                "about A/B Street" => {
                    return Transition::Push(PopupMsg::new_state(ctx, "TODO", vec!["TODO"]));
                }
                "Bike Master Plan" => {
                    return Transition::Push(PopupMsg::new_state(ctx, "TODO", vec!["TODO"]));
                }
                "Edit network" => {
                    return Transition::Push(PopupMsg::new_state(ctx, "TODO", vec!["TODO"]));
                }
                _ => unreachable!(),
            }
        }

        if let Outcome::Clicked(x) = self.legend.event(ctx) {
            return Transition::Push(match x.as_ref() {
                "change map" => CityPicker::new_state(
                    ctx,
                    app,
                    Box::new(|ctx, app| {
                        Transition::Multi(vec![Transition::Pop, Transition::Replace(ExploreMap::new_state(ctx, app))])
                    }),
                ),
                // TODO Add physical picture examples
                "highway" => PopupMsg::new_state(ctx, "Highways", vec!["Unless there's a separate trail (like on the 520 or I90 bridge), highways aren't accessible to biking"]),
                "major street" => PopupMsg::new_state(ctx, "Major streets", vec!["Arterials have more traffic, but are often where businesses are located"]),
                "minor street" => PopupMsg::new_state(ctx, "Minor streets", vec!["Local streets have a low volume of traffic and are usually comfortable for biking, even without dedicated infrastructure"]),
                "trail" => PopupMsg::new_state(ctx, "Trails", vec!["Trails like the Burke Gilman are usually well-separated from vehicle traffic. The space is usually shared between people walking, cycling, and rolling."]),
                "protected bike lane" => PopupMsg::new_state(ctx, "Protected bike lanes", vec!["Bike lanes separated from vehicle traffic by physical barriers or a few feet of striping"]),
                "painted bike lane" => PopupMsg::new_state(ctx, "Painted bike lanes", vec!["Bike lanes without any separation from vehicle traffic. Often uncomfortably close to the \"door zone\" of parked cars."]),
                "Stay Healthy Street / greenway" => PopupMsg::new_state(ctx, "Stay Healthy Streets and neighborhood greenways", vec!["Residential streets with additional signage and light barriers. These are intended to be low traffic, dedicated for people walking and biking."]),
                _ => unreachable!(),
            });
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.top_panel.draw(g);
        self.legend.draw(g);
    }
}

fn make_top_panel(ctx: &mut EventCtx) -> Panel {
    Panel::new_builder(Widget::row(vec![
        ctx.style()
            .btn_plain
            .btn()
            .image_path("system/assets/pregame/logo.svg")
            .image_dims(50.0)
            .build_widget(ctx, "about A/B Street"),
        // TODO Tab style?
        ctx.style()
            .btn_solid_primary
            .text("Today")
            .disabled(true)
            .build_def(ctx)
            .centered_vert(),
        ctx.style()
            .btn_solid_primary
            .text("Bike Master Plan")
            .build_def(ctx)
            .centered_vert(),
        ctx.style()
            .btn_solid_primary
            .icon_text("system/assets/tools/pencil.svg", "Edit network")
            .build_def(ctx)
            .centered_vert(),
    ]))
    .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
    .build(ctx)
}

fn make_legend(ctx: &mut EventCtx, app: &App) -> Panel {
    Panel::new_builder(Widget::col(vec![
        Widget::custom_row(vec![
            Line("Bike Network")
                .small_heading()
                .into_widget(ctx)
                .margin_right(18),
            ctx.style()
                .btn_popup_icon_text(
                    "system/assets/tools/map.svg",
                    nice_map_name(app.primary.map.get_name()),
                )
                .hotkey(lctrl(Key::L))
                .build_widget(ctx, "change map")
                .margin_right(8),
        ]),
        legend(ctx, app.cs.unzoomed_highway, "highway"),
        legend(ctx, app.cs.unzoomed_arterial, "major street"),
        legend(ctx, app.cs.unzoomed_residential, "minor street"),
        legend(ctx, app.cs.unzoomed_trail, "trail"),
        // TODO Untuned colors
        legend(ctx, Color::hex("#74B0FC"), "protected bike lane"),
        legend(ctx, Color::GREEN, "painted bike lane"),
        legend(ctx, Color::BLUE, "Stay Healthy Street / greenway"),
        // TODO Distinguish door-zone bike lanes?
        // TODO Call out bike turning boxes?
        // TODO Call out bike signals?
    ]))
    .aligned(HorizontalAlignment::Right, VerticalAlignment::Bottom)
    .build(ctx)
}

fn legend(ctx: &mut EventCtx, color: Color, label: &str) -> Widget {
    let radius = 15.0;
    Widget::row(vec![
        GeomBatch::from(vec![(
            color,
            Circle::new(Pt2D::new(radius, radius), Distance::meters(radius)).to_polygon(),
        )])
        .into_widget(ctx)
        .centered_vert(),
        ctx.style()
            .btn_plain
            .text(label)
            .build_def(ctx)
            .centered_vert(),
    ])
}
