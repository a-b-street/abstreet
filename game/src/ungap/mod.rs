use geom::{Circle, Distance, Pt2D};
use map_gui::tools::{nice_map_name, CityPicker, PopupMsg};
use map_gui::ID;
use map_model::RoadID;
use widgetry::{
    lctrl, Cached, Color, Drawable, EventCtx, GeomBatch, GeomBatchStack, GfxCtx,
    HorizontalAlignment, Key, Line, Outcome, Panel, State, Text, VerticalAlignment, Widget,
};

use crate::app::{App, Transition};
use crate::common::Warping;

pub struct ExploreMap {
    top_panel: Panel,
    legend: Panel,
    tooltip: Cached<RoadID, (RoadID, Drawable, Drawable)>,
}

impl ExploreMap {
    pub fn new_state(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
        Box::new(ExploreMap {
            top_panel: make_top_panel(ctx),
            legend: make_legend(ctx, app),
            tooltip: Cached::new(),
        })
    }
}

impl State<App> for ExploreMap {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();

        if ctx.redo_mouseover() {
            let road = match app.mouseover_unzoomed_roads_and_intersections(ctx) {
                Some(ID::Road(r)) => Some(r),
                _ => None,
            };
            self.tooltip.update(road, |r| make_tooltip(ctx, app, r));
        }

        if let Some((r, _, _)) = self.tooltip.value() {
            if ctx.normal_left_click() {
                let r = *r;
                self.tooltip.clear();
                return Transition::Push(Warping::new_state(
                    ctx,
                    app.primary.map.get_r(r).center_pts.middle(),
                    Some(10.0),
                    None,
                    &mut app.primary,
                ));
            }
        }

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
        if let Some((_, draw_on_map, draw_tooltip)) = self.tooltip.value() {
            g.redraw(draw_on_map);

            // Like fork_screenspace, but centered by the cursor
            g.fork(Pt2D::new(0.0, 0.0), g.canvas.get_cursor(), 1.0, None);
            g.redraw(draw_tooltip);
            g.unfork();
        }
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

// Returns a batch to draw directly on the map, and another to draw as a tooltip.
fn make_tooltip(ctx: &mut EventCtx, app: &App, r: RoadID) -> (RoadID, Drawable, Drawable) {
    let mut map_batch = GeomBatch::new();
    let road = app.primary.map.get_r(r);
    map_batch.push(
        Color::BLACK.alpha(0.5),
        road.get_thick_polygon(&app.primary.map),
    );

    let mut screen_batch = GeomBatch::new();
    for (l, _, _) in road.lanes_ltr() {
        screen_batch.append(app.primary.draw_map.get_l(l).render(ctx, app));
    }
    screen_batch.append(app.primary.draw_map.get_r(r).render_center_line(app));

    screen_batch = screen_batch.autocrop();
    let bounds = screen_batch.get_bounds();
    let fit_dims = 300.0;
    let zoom = (fit_dims / bounds.width()).min(fit_dims / bounds.height());
    screen_batch = screen_batch.scale(zoom);

    let label = Text::from(Line(road.get_name(app.opts.language.as_ref())).small_heading())
        .render_autocropped(ctx);
    screen_batch = GeomBatchStack::vertical(vec![label, screen_batch]).batch();
    screen_batch = magnifying_glass(screen_batch);

    (r, map_batch.upload(ctx), screen_batch.upload(ctx))
}

fn magnifying_glass(batch: GeomBatch) -> GeomBatch {
    let bounds = batch.get_bounds();
    // TODO The radius isn't guaranteed to fit...
    let radius = Distance::meters(1.3 * bounds.width().max(bounds.height())) / 2.0;
    let circle = Circle::new(bounds.center(), radius);
    let mut new_batch = GeomBatch::new();
    new_batch.push(Color::WHITE, circle.to_polygon());
    if let Ok(p) = circle.to_outline(Distance::meters(3.0)) {
        new_batch.push(Color::BLACK, p);
    }
    new_batch.append(batch);
    new_batch
}
