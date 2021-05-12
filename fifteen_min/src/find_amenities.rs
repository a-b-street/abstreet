use abstutil::Timer;
use geom::Percent;
use map_model::AmenityType;
use map_gui::tools::ChooseSomething;
use widgetry::{
    Drawable, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Panel, Choice,
    SimpleState, State, TextExt, Toggle, Transition, VerticalAlignment, Widget,
};

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
            Box::new(|at, ctx, app| {
                let multi_isochrone = create_multi_isochrone(ctx, app, at, options);
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
        ]))
            .aligned(HorizontalAlignment::RightInset, VerticalAlignment::TopInset)
            .build(ctx);

        let batch = isochrone.draw_isochrone(app);

        SimpleState::new(
            panel,
            Box::new(Results {
                draw: ctx.upload(batch),
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

    fn other_event(&mut self, ctx: &mut EventCtx, _: &mut App) -> Transition<App> {
        ctx.canvas_movement();
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        g.redraw(&self.draw);
    }
}
