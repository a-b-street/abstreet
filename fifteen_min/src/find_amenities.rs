use map_gui::tools::{ChooseSomething, ColorLegend};
use map_gui::ID;
use map_model::AmenityType;
use widgetry::{
    Cached, Choice, Color, Drawable, EventCtx, GfxCtx, HorizontalAlignment, Line, Panel,
    SimpleState, State, TextExt, Transition, VerticalAlignment, Widget,
};

use crate::isochrone::{draw_isochrone, BorderIsochrone, Isochrone, Options};
use crate::viewer::{draw_star, HoverKey, HoverOnBuilding};
use crate::App;

/// Calculate isochrones around each amenity on a map and merge them together using the min value
pub struct FindAmenity;

impl FindAmenity {
    pub fn new_state(ctx: &mut EventCtx, options: Options) -> Box<dyn State<App>> {
        ChooseSomething::new_state(
            ctx,
            "Choose an amenity",
            AmenityType::all()
                .into_iter()
                .map(|at| Choice::new(at.to_string(), at))
                .collect(),
            Box::new(move |at, ctx, app| {
                let multi_isochrone = create_multi_isochrone(ctx, app, at, options.clone());
                let border_isochrone = create_border_isochrone(ctx, app, options);
                return Transition::Replace(Results::new_state(
                    ctx,
                    app,
                    multi_isochrone,
                    border_isochrone,
                    at,
                ));
            }),
        )
    }
}

/// For every one of the requested amenity on the map, draw an isochrone
fn create_multi_isochrone(
    ctx: &mut EventCtx,
    app: &App,
    category: AmenityType,
    options: Options,
) -> Isochrone {
    let map = &app.map;
    // For a category, find all matching stores
    let mut stores = Vec::new();
    for b in map.all_buildings() {
        if b.has_amenity(category) {
            stores.push(b.id);
        }
    }
    Isochrone::new(ctx, app, stores, options)
}

/// Draw an isochrone from every intersection border
fn create_border_isochrone(ctx: &mut EventCtx, app: &App, options: Options) -> BorderIsochrone {
    let mut all_intersections = Vec::new();
    for i in app.map.all_intersections() {
        if i.is_border() {
            all_intersections.push(i.id);
        }
    }
    BorderIsochrone::new(ctx, app, all_intersections, options)
}

struct Results {
    draw: Drawable,
    isochrone: Isochrone,
    hovering_on_bldg: Cached<HoverKey, HoverOnBuilding>,
}

impl Results {
    fn new_state(
        ctx: &mut EventCtx,
        app: &App,
        isochrone: Isochrone,
        border_isochrone: BorderIsochrone,
        category: AmenityType,
    ) -> Box<dyn State<App>> {
        let panel = Panel::new_builder(Widget::col(vec![
            Widget::row(vec![
                Line(format!("{} within 15 minutes", category))
                    .small_heading()
                    .into_widget(ctx),
                ctx.style().btn_close_widget(ctx),
            ]),
            format!("{} matching amenities", isochrone.start.len()).text_widget(ctx),
            ColorLegend::categories(
                ctx,
                vec![
                    (Color::GREEN, "5 mins"),
                    (Color::ORANGE, "10 mins"),
                    (Color::RED, "15 mins"),
                ],
            ),
            ColorLegend::row(
                ctx,
                Color::rgb(0, 0, 0).alpha(0.3),
                "< 15 mins from border (amenity could exist off map)",
            ),
        ]))
        .aligned(HorizontalAlignment::RightInset, VerticalAlignment::TopInset)
        .build(ctx);

        let mut batch = draw_isochrone(
            app,
            &border_isochrone.time_to_reach_building,
            &border_isochrone.thresholds,
            &border_isochrone.colors,
            border_isochrone.options.params,
        );
        batch.append(draw_isochrone(
            app,
            &isochrone.time_to_reach_building,
            &isochrone.thresholds,
            &isochrone.colors,
            isochrone.options.params,
        ));
        for &start in &isochrone.start {
            batch.append(draw_star(ctx, app.map.get_b(start)));
        }

        <dyn SimpleState<_>>::new_state(
            panel,
            Box::new(Results {
                draw: ctx.upload(batch),
                isochrone,
                hovering_on_bldg: Cached::new(),
            }),
        )
    }
}

impl SimpleState<App> for Results {
    fn on_click(&mut self, _: &mut EventCtx, _: &mut App, x: &str, _: &Panel) -> Transition<App> {
        match x {
            "close" => Transition::Pop,
            _ => unreachable!(),
        }
    }

    fn other_event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition<App> {
        ctx.canvas_movement();

        if ctx.redo_mouseover() {
            let isochrone = &self.isochrone;
            self.hovering_on_bldg
                .update(HoverOnBuilding::key(ctx, app), |key| {
                    HoverOnBuilding::value(ctx, app, key, isochrone)
                });
            // Also update this to conveniently get an outline drawn. Note we don't want to do this
            // inside the callback above, because it doesn't run when the key becomes None.
            app.current_selection = self.hovering_on_bldg.key().map(|(b, _)| ID::Building(b));
        }
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        g.redraw(&self.isochrone.draw);

        if let Some(ref hover) = self.hovering_on_bldg.value() {
            g.draw_mouse_tooltip(hover.tooltip.clone());
            g.redraw(&hover.drawn_route);
        }
        g.redraw(&self.draw);
    }
}
