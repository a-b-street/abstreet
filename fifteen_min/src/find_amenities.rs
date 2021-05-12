use map_model::AmenityType;
use map_gui::tools::{ChooseSomething, ColorLegend};
use map_gui::ID;
use widgetry::{
    Drawable, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Panel, Choice, Color,
    SimpleState, State, Transition, VerticalAlignment, Widget, Cached,
};

use crate::viewer::{HoverOnBuilding, HoverKey, draw_unwalkable_roads, draw_star};
use crate::isochrone::{Options, Isochrone};
use crate::App;

/// Calculate isochrones around each amenity on a map and merge them together using the min value
pub struct FindAmenity {
    options: Options,
}


impl FindAmenity {
    pub fn new(ctx: &mut EventCtx, options: Options) -> Box<dyn State<App>> {
        ChooseSomething::new(
            ctx,
            "Choose an amenity",
            AmenityType::all()
                    .into_iter()
                    .map(|at| Choice::new(at.to_string(), at))
                    .collect(),
            Box::new(move |at, ctx, app| {
                let multi_isochrone = create_multi_isochrone(ctx, app, at, options.clone());
                return Transition::Push(Results::new(ctx, app, multi_isochrone, at));
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
    Isochrone::new(ctx, app, stores, options.clone())
}

struct Results {
    draw: Drawable,
    isochrone: Isochrone,
    hovering_on_bldg: Cached<HoverKey, HoverOnBuilding>,
    // TODO Can't use Cached due to a double borrow
    hovering_on_category: Option<(AmenityType, Drawable)>,
    draw_unwalkable_roads: Drawable,
}

impl Results {
    fn new(
        ctx: &mut EventCtx,
        app: &App,
        isochrone: Isochrone,
        category: AmenityType,
    ) -> Box<dyn State<App>> {

        let panel = Panel::new(Widget::col(vec![
            Line(format!("{} within 15 minutes", category))
                .small_heading()
                .into_widget(ctx),
            ctx.style()
                .btn_outline
                .text("Back")
                .hotkey(Key::Escape)
                .build_def(ctx),
                ColorLegend::categories(
                    ctx,
                    vec![
                        (Color::GREEN, "5 mins"),
                        (Color::ORANGE, "10 mins"),
                        (Color::RED, "15 mins"),
                    ],
                )
        ]))
            .aligned(HorizontalAlignment::RightInset, VerticalAlignment::TopInset)
            .build(ctx);

    


        let mut batch = isochrone.draw_isochrone(app);
        for &start in &isochrone.start {
            batch.append(draw_star(ctx, app.map.get_b(start)));
        }

        let draw_unwalkable_roads = draw_unwalkable_roads(ctx, app, &isochrone.options);
        
        SimpleState::new(
            panel,
            Box::new(Results {
                draw: ctx.upload(batch),
                isochrone: isochrone,
                hovering_on_bldg: Cached::new(),
                hovering_on_category: None,
                draw_unwalkable_roads,

            }),
        )
    }
}

impl SimpleState<App> for Results {
    fn on_click(&mut self, _: &mut EventCtx, _: &mut App, x: &str, _: &Panel) -> Transition<App> {
        match x {
            "Back" => Transition::Pop,
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
            self.hovering_on_category = None;
        }
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        g.redraw(&self.isochrone.draw);
        g.redraw(&self.draw_unwalkable_roads);
    
        if let Some(ref hover) = self.hovering_on_bldg.value() {
            g.draw_mouse_tooltip(hover.tooltip.clone());
            g.redraw(&hover.drawn_route);
        }
        g.redraw(&self.draw);
    }
}

