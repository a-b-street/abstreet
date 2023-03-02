use std::collections::HashMap;

use crate::App;
use abstutil::{prettyprint_usize, Counter, Timer};
use geom::Percent;
use map_gui::tools::grey_out_map;
use map_model::connectivity::Spot;
use map_model::{AmenityType, BuildingID};
use widgetry::tools::{PopupMsg, URLManager};
use widgetry::{
    Color, DrawBaselayer, Drawable, EventCtx, GeomBatch, GfxCtx, Key, Line, Outcome, Panel,
    SimpleState, State, TextExt, Toggle, Transition, Widget,
};

use crate::isochrone::Options;
use crate::{common, render};

/// Ask what types of amenities are necessary to be within a walkshed, then rank every house with
/// how many of those needs are satisfied.
pub struct ScoreHomes;

impl ScoreHomes {
    pub fn new_state(ctx: &mut EventCtx) -> Box<dyn State<App>> {
        let panel = Panel::new_builder(Widget::col(vec![
            Widget::row(vec![Line("Calculate acces scores")
                .small_heading()
                .into_widget(ctx)]),
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
                .text("Calculate")
                .hotkey(Key::Enter)
                .build_def(ctx),
        ]))
        .build(ctx);

        <dyn SimpleState<_>>::new_state(panel, Box::new(ScoreHomes))
    }
}

impl SimpleState<App> for ScoreHomes {
    fn on_click(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        x: &str,
        panel: &mut Panel,
    ) -> Transition<App> {
        match x {
            "Calculate" => {
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

                return Transition::Multi(vec![
                    Transition::Pop,
                    Transition::Replace(Results::new_state(ctx, app, amenities)),
                ]);
            }
            _ => unreachable!(),
        }
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        grey_out_map(g, app);
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::PreviousState
    }
}

/// For every house in the map, return the percent of amenities located within a 15min walkshed. A
/// single matching business per category is enough to count as satisfied.
fn score_houses(
    app: &App,
    amenities: Vec<AmenityType>,
    timer: &mut Timer,
) -> HashMap<BuildingID, Percent> {
    let num_categories = amenities.len();
    let mut satisfied_per_bldg: Counter<BuildingID> = Counter::new();

    let map = &app.map;
    let movement_opts = &app.session.movement;
    for times in timer.parallelize("find houses close to amenities", amenities, |category| {
        // For each category, find all matching stores
        let mut stores = Vec::new();
        for b in map.all_buildings() {
            if b.has_amenity(category) {
                stores.push(Spot::Building(b.id));
            }
        }
        movement_opts.clone().times_from(map, stores)
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
    panel: Panel,
    draw_houses: Drawable,
    amenities: Vec<AmenityType>,
    draw_unwalkable_roads: Drawable,
}

impl Results {
    fn new_state(
        ctx: &mut EventCtx,
        app: &App,
        amenities: Vec<AmenityType>,
    ) -> Box<dyn State<App>> {
        let draw_unwalkable_roads = render::draw_unwalkable_roads(ctx, app);

        let scores = ctx.loading_screen("search for houses", |_, timer| {
            score_houses(app, amenities.clone(), timer)
        });

        // TODO Show imperfect matches with different colors.
        let mut batch = GeomBatch::new();
        let mut count = 0;
        for (b, pct) in scores {
            if pct == Percent::int(100) {
                batch.push(Color::RED, app.map.get_b(b).polygon.clone());
                count += 1;
            }
        }

        let panel = build_panel(ctx, app, &amenities, count);

        Box::new(Self {
            draw_unwalkable_roads,
            panel,
            draw_houses: ctx.upload(batch),
            amenities,
        })
    }
}

impl State<App> for Results {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition<App> {
        // Allow panning and zooming
        if ctx.canvas_movement() {
            URLManager::update_url_cam(ctx, app.map.get_gps_bounds());
        }

        match self.panel.event(ctx) {
            Outcome::Clicked(x) => {
                if x == "change scoring criteria" {
                    return Transition::Push(ScoreHomes::new_state(ctx));
                }
                return common::on_click(ctx, app, &x);
            }
            Outcome::Changed(_) => {
                app.session = Options {
                    movement: common::options_from_controls(&self.panel),
                    thresholds: Options::default_thresholds(),
                };
                return Transition::Replace(Self::new_state(ctx, app, self.amenities.clone()));
            }
            _ => {}
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        g.redraw(&self.draw_unwalkable_roads);
        g.redraw(&self.draw_houses);
        self.panel.draw(g);
    }
}

fn build_panel(ctx: &mut EventCtx, app: &App, amenities: &Vec<AmenityType>, count: usize) -> Panel {
    let contents = vec![
        "What homes are within 15 minutes away?".text_widget(ctx),
        format!(
            "Containing at least 1 of each: {}",
            amenities
                .iter()
                .map(|x| x.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        )
        .text_widget(ctx),
        format!("{} houses match", prettyprint_usize(count)).text_widget(ctx),
        ctx.style()
            .btn_outline
            .text("change scoring criteria")
            .build_def(ctx),
    ];

    common::build_panel(ctx, app, common::Mode::ScoreHomes, Widget::col(contents))
}
