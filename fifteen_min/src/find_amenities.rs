use abstutil::Timer;
use geom::Percent;
use map_model::AmenityType;
use widgetry::{
    Drawable, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Panel,
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
        let panel = Panel::new(Widget::col(vec![
            Widget::row(vec![
                Line("Find where amenities exist.")
                    .small_heading()
                    .into_widget(ctx),
                ctx.style().btn_close_widget(ctx),
            ]),
            // TODO Adjust text to say bikeshed, or otherwise reflect the options chosen
            "Choose an amenity.".text_widget(ctx),
            Widget::custom_row(
                AmenityType::all()
                    .into_iter()
                    .map(|at| Toggle::switch(ctx, &at.to_string(), None, false))
                    .collect(),
            )
                .flex_wrap(ctx, Percent::int(50)),
            ctx.style()
                .btn_solid_primary
                .text("Search")
                .hotkey(Key::Enter)
                .build_def(ctx),
        ]))
            .build(ctx);

        SimpleState::new(panel, Box::new(FindAmenity { options }))
    }
}

impl SimpleState<App> for FindAmenity {
    fn on_click(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        x: &str,
        panel: &Panel,
    ) -> Transition<App> {
        match x {
            "close" => Transition::Pop,
            "Search" => {
                let amenities: Vec<AmenityType> = AmenityType::all()
                    .into_iter()
                    .filter(|at| panel.is_checked(&at.to_string()))
                    .collect();

                let isochrones = create_isochrones(ctx, app, amenities[0], self.options.clone());
                return Transition::Push(Results::new(ctx, app, isochrones, amenities[0]));
            }
            _ => unreachable!(),
        }
    }
}

/// For every one of the requested amenity on the map, draw an isochrone
fn create_isochrones(
    ctx: &mut EventCtx,
    app: &App,
    category: AmenityType,
    options: Options,
) -> Vec<Isochrone> {

    let map = &app.map;
    let mut isochrones: Vec<Isochrone> = Vec::new();
    for b in map.all_buildings() {
        if b.has_amenity(category) {
            isochrones.push(Isochrone::new(ctx, app, b.id, options.clone()));
        }
    }
    isochrones
}

struct Results {
    draw: Drawable,
}

impl Results {
    fn new(
        ctx: &mut EventCtx,
        app: &App,
        isochrones: Vec<Isochrone>,
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

        // TODO make this draw more than one
        let batch = isochrones[0].draw_isochrone(app);

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
