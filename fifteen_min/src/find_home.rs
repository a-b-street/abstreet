use std::collections::HashMap;

use crate::App;
use abstutil::{prettyprint_usize, Counter, Timer};
use geom::Percent;
use map_gui::tools::PopupMsg;
use map_model::connectivity::Spot;
use map_model::{AmenityType, BuildingID};
use widgetry::{
    Color, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key, Line, Panel,
    SimpleState, State, TextExt, Toggle, Transition, VerticalAlignment, Widget,
};

use crate::isochrone::Options;

/// Ask what types of amenities are necessary to be within a walkshed, then rank every house with
/// how many of those needs are satisfied.
pub struct FindHome {
    options: Options,
}

impl FindHome {
    pub fn new_state(ctx: &mut EventCtx, options: Options) -> Box<dyn State<App>> {
        let panel = Panel::new_builder(Widget::col(vec![
            Widget::row(vec![
                Line("Find your walkable home")
                    .small_heading()
                    .into_widget(ctx),
                ctx.style().btn_close_widget(ctx),
            ]),
            // TODO Adjust text to say bikeshed, or otherwise reflect the options chosen
            "Select the types of businesses you want within a 15 minute walkshed.".text_widget(ctx),
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

        <dyn SimpleState<_>>::new_state(panel, Box::new(FindHome { options }))
    }
}

impl SimpleState<App> for FindHome {
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
                if amenities.is_empty() {
                    return Transition::Push(PopupMsg::new_state(
                        ctx,
                        "No amenities selected",
                        vec!["Please select at least one amenity that you want in your walkshd"],
                    ));
                }

                let scores = ctx.loading_screen("search for houses", |_, timer| {
                    score_houses(app, amenities.clone(), self.options.clone(), timer)
                });
                return Transition::Push(Results::new_state(ctx, app, scores, amenities));
            }
            _ => unreachable!(),
        }
    }
}

/// For every house in the map, return the percent of amenities located within a 15min walkshed. A
/// single matching business per category is enough to count as satisfied.
fn score_houses(
    app: &App,
    amenities: Vec<AmenityType>,
    options: Options,
    timer: &mut Timer,
) -> HashMap<BuildingID, Percent> {
    let num_categories = amenities.len();
    let mut satisfied_per_bldg: Counter<BuildingID> = Counter::new();

    let map = &app.map;
    for times in timer.parallelize("find houses close to amenities", amenities, |category| {
        // For each category, find all matching stores
        let mut stores = Vec::new();
        for b in map.all_buildings() {
            if b.has_amenity(category) {
                stores.push(Spot::Building(b.id));
            }
        }
        options.clone().times_from(map, stores)
    }) {
        for (b, _) in times {
            satisfied_per_bldg.inc(b);
        }
    }

    let mut scores = HashMap::new();
    for (b, cnt) in satisfied_per_bldg.consume() {
        scores.insert(b, Percent::of(cnt, num_categories));
    }
    scores
}

// TODO Show the matching amenities.
// TODO As you hover over a building, show the nearest amenity of each type
struct Results {
    draw_houses: Drawable,
}

impl Results {
    fn new_state(
        ctx: &mut EventCtx,
        app: &App,
        scores: HashMap<BuildingID, Percent>,
        amenities: Vec<AmenityType>,
    ) -> Box<dyn State<App>> {
        // TODO Show imperfect matches with different colors.
        let mut batch = GeomBatch::new();
        let mut count = 0;
        for (b, pct) in scores {
            if pct == Percent::int(100) {
                batch.push(Color::RED, app.map.get_b(b).polygon.clone());
                count += 1;
            }
        }

        let panel = Panel::new_builder(Widget::col(vec![
            Line("Results for your walkable home")
                .small_heading()
                .into_widget(ctx),
            // TODO Adjust text to say bikeshed, or otherwise reflect the options chosen
            format!("{} houses match", prettyprint_usize(count)).text_widget(ctx),
            format!(
                "Containing at least 1 of each: {}",
                amenities
                    .into_iter()
                    .map(|x| x.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
            .text_widget(ctx),
            ctx.style()
                .btn_outline
                .text("Back")
                .hotkey(Key::Escape)
                .build_def(ctx),
        ]))
        .aligned(HorizontalAlignment::RightInset, VerticalAlignment::TopInset)
        .build(ctx);

        <dyn SimpleState<_>>::new_state(
            panel,
            Box::new(Results {
                draw_houses: ctx.upload(batch),
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
        g.redraw(&self.draw_houses);
    }
}
