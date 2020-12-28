use std::collections::{HashMap, HashSet};

use abstutil::{Counter, Parallelism, Timer};
use geom::Percent;
use map_gui::tools::PopupMsg;
use map_model::{AmenityType, BuildingID};
use widgetry::{
    Btn, Checkbox, Color, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key, Line,
    Panel, SimpleState, State, TextExt, Transition, VerticalAlignment, Widget,
};

use crate::isochrone::Options;
use crate::App;

/// Ask what types of amenities are necessary to be within a walkshed, then rank every house with
/// how many of those needs are satisfied.
pub struct FindHome {
    options: Options,
}

impl FindHome {
    pub fn new(ctx: &mut EventCtx, options: Options) -> Box<dyn State<App>> {
        let panel = Panel::new(Widget::col(vec![
            Widget::row(vec![
                Line("Find your walkable home").small_heading().draw(ctx),
                Btn::close(ctx),
            ]),
            // TODO Adjust text to say bikeshed, or otherwise reflect the options chosen
            "Select the types of businesses you want within a 15 minute walkshed.".draw_text(ctx),
            Widget::custom_row(
                AmenityType::all()
                    .into_iter()
                    .map(|at| Checkbox::switch(ctx, at.to_string(), None, false))
                    .collect(),
            )
            .flex_wrap(ctx, Percent::int(50)),
            Btn::text_bg2("Search").build_def(ctx, Key::Enter),
        ]))
        .build(ctx);

        SimpleState::new(panel, Box::new(FindHome { options }))
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
                    return Transition::Push(PopupMsg::new(
                        ctx,
                        "No amenities selected",
                        vec!["Please select at least one amenity that you want in your walkshd"],
                    ));
                }

                let scores = ctx.loading_screen("search for houses", |_, timer| {
                    score_houses(app, amenities.clone(), self.options.clone(), timer)
                });
                return Transition::Push(Results::new(ctx, app, scores, amenities));
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
    let mut satisfied_per_bldg: Counter<BuildingID> = Counter::new();

    // There may be more clever ways to calculate this, but we'll take the brute-force approach of
    // calculating the walkshed from every matching business.
    let num_categories = amenities.len();
    for category in amenities {
        let mut stores: HashSet<BuildingID> = HashSet::new();
        for b in app.map.all_buildings() {
            if b.has_amenity(category) {
                stores.insert(b.id);
            }
        }

        let mut houses: HashSet<BuildingID> = HashSet::new();
        let map = &app.map;
        for times in timer.parallelize(
            &format!("find houses close to {}", category),
            Parallelism::Fastest,
            stores.into_iter().collect(),
            |b| options.clone().time_to_reach_building(map, b),
        ) {
            for (b, _) in times {
                houses.insert(b);
            }
        }

        for b in houses {
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
    fn new(
        ctx: &mut EventCtx,
        app: &App,
        scores: HashMap<BuildingID, Percent>,
        amenities: Vec<AmenityType>,
    ) -> Box<dyn State<App>> {
        let panel = Panel::new(Widget::col(vec![
            Line("Results for your walkable home")
                .small_heading()
                .draw(ctx),
            // TODO Adjust text to say bikeshed, or otherwise reflect the options chosen
            "Here are all of the matching houses.".draw_text(ctx),
            format!(
                "Containing at least 1 of each: {}",
                amenities
                    .into_iter()
                    .map(|x| x.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
            .draw_text(ctx),
            Btn::text_bg2("Back").build_def(ctx, Key::Escape),
        ]))
        .aligned(HorizontalAlignment::RightInset, VerticalAlignment::TopInset)
        .build(ctx);

        // TODO Show imperfect matches with different colors.
        let mut batch = GeomBatch::new();
        for (b, pct) in scores {
            if pct == Percent::int(100) {
                batch.push(Color::RED, app.map.get_b(b).polygon.clone());
            }
        }

        SimpleState::new(
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
