use std::collections::BTreeSet;

use crate::App;
use abstutil::{prettyprint_usize, Counter, MultiMap, Timer};
use geom::Percent;
use map_gui::tools::grey_out_map;
use map_model::connectivity::Spot;
use map_model::{AmenityType, BuildingID};
use widgetry::tools::{ColorLegend, PopupMsg, URLManager};
use widgetry::{
    Color, DrawBaselayer, Drawable, EventCtx, GeomBatch, GfxCtx, Key, Line, Outcome, Panel,
    SimpleState, State, Text, TextExt, Toggle, Transition, Widget,
};

use crate::isochrone::Options;
use crate::{common, render};

/// Ask what types of amenities are necessary to be within a walkshed, then rank every house with
/// how many of those needs are satisfied.
pub struct ScoreHomes;

impl ScoreHomes {
    pub fn new_state(
        ctx: &mut EventCtx,
        app: &App,
        amenities: Vec<AmenityType>,
    ) -> Box<dyn State<App>> {
        let amenities_present = app.map.get_available_amenity_types();
        let mut toggles = Vec::new();
        let mut missing = Vec::new();
        for at in AmenityType::all() {
            if amenities_present.contains(&at) {
                toggles.push(Toggle::switch(
                    ctx,
                    &at.to_string(),
                    None,
                    amenities.contains(&at),
                ));
            } else {
                missing.push(at.to_string());
            }
        }

        let panel = Panel::new_builder(Widget::col(vec![
            Widget::row(vec![Line("Calculate acces scores")
                .small_heading()
                .into_widget(ctx)]),
            // TODO Adjust text to say bikeshed, or otherwise reflect the options chosen
            "Select the types of businesses you want within a 15 minute walkshed.".text_widget(ctx),
            Widget::row(vec![
                ctx.style().btn_outline.text("Enable all").build_def(ctx),
                ctx.style().btn_outline.text("Disable all").build_def(ctx),
            ]),
            Widget::custom_row(toggles).flex_wrap(ctx, Percent::int(50)),
            ctx.style()
                .btn_solid_primary
                .text("Calculate")
                .hotkey(Key::Enter)
                .build_def(ctx),
            Text::from(
                Line(format!(
                    "These amenities aren't present in this map: {}",
                    missing.join(", ")
                ))
                .secondary(),
            )
            .wrap_to_pct(ctx, 50)
            .into_widget(ctx),
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
            "Enable all" => {
                return Transition::Replace(Self::new_state(
                    ctx,
                    app,
                    app.map.get_available_amenity_types().into_iter().collect(),
                ));
            }
            "Disable all" => {
                return Transition::Replace(Self::new_state(ctx, app, Vec::new()));
            }
            "Calculate" => {
                let amenities: Vec<AmenityType> = AmenityType::all()
                    .into_iter()
                    .filter(|at| panel.maybe_is_checked(&at.to_string()).unwrap_or(false))
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

/// For every house in the map, return the number of amenity types located within a 15min walkshed.
/// A single matching business per category is enough to count as satisfied.
fn score_houses_by_one_match(
    app: &App,
    amenities: Vec<AmenityType>,
    timer: &mut Timer,
) -> (Counter<BuildingID>, MultiMap<AmenityType, BuildingID>) {
    let mut satisfied_per_bldg: Counter<BuildingID> = Counter::new();
    let mut amenities_reachable = MultiMap::new();

    let map = &app.map;
    let movement_opts = &app.session.movement;
    for (category, stores, times) in
        timer.parallelize("find houses close to amenities", amenities, |category| {
            // For each category, find all matching stores
            let mut stores = BTreeSet::new();
            let mut spots = Vec::new();
            for b in map.all_buildings() {
                if b.has_amenity(category) {
                    stores.insert(b.id);
                    spots.push(Spot::Building(b.id));
                }
            }
            (
                category,
                stores,
                movement_opts.clone().times_from(map, spots),
            )
        })
    {
        amenities_reachable.set(category, stores);
        for (b, _) in times {
            satisfied_per_bldg.inc(b);
        }
    }

    (satisfied_per_bldg, amenities_reachable)
}

// TODO Show the matching amenities.
// TODO As you hover over a building, show the nearest amenity of each type
struct Results {
    panel: Panel,
    draw_houses: Drawable,
    amenities: Vec<AmenityType>,
    amenities_reachable: MultiMap<AmenityType, BuildingID>,
    draw_unwalkable_roads: Drawable,
    hovering_on_category: common::HoverOnCategory,
}

impl Results {
    fn new_state(
        ctx: &mut EventCtx,
        app: &App,
        amenities: Vec<AmenityType>,
    ) -> Box<dyn State<App>> {
        let draw_unwalkable_roads = render::draw_unwalkable_roads(ctx, app);

        assert!(!amenities.is_empty());
        let (scores, amenities_reachable) = ctx.loading_screen("search for houses", |_, timer| {
            score_houses_by_one_match(app, amenities.clone(), timer)
        });

        let mut batch = GeomBatch::new();
        let mut matches_all = 0;

        for (b, count) in scores.consume() {
            if count == amenities.len() {
                matches_all += 1;
            }
            let color = app
                .cs
                .good_to_bad_red
                .eval((count as f64) / (amenities.len() as f64));
            batch.push(color, app.map.get_b(b).polygon.clone());
        }

        let panel = build_panel(ctx, app, &amenities, &amenities_reachable, matches_all);

        Box::new(Self {
            draw_unwalkable_roads,
            panel,
            draw_houses: ctx.upload(batch),
            amenities,
            amenities_reachable,
            hovering_on_category: common::HoverOnCategory::new(Color::YELLOW),
        })
    }
}

impl State<App> for Results {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition<App> {
        // Allow panning and zooming
        if ctx.canvas_movement() {
            URLManager::update_url_cam(ctx, app.map.get_gps_bounds());
        }

        if ctx.redo_mouseover() {
            self.hovering_on_category.update_on_mouse_move(
                ctx,
                app,
                &self.panel,
                &self.amenities_reachable,
            );
        }

        match self.panel.event(ctx) {
            Outcome::Clicked(x) => {
                if x == "change scoring criteria" {
                    return Transition::Push(ScoreHomes::new_state(
                        ctx,
                        app,
                        self.amenities.clone(),
                    ));
                } else if x.starts_with("businesses: ") {
                    // TODO Use ExploreAmenitiesDetails, but omit duration
                    return Transition::Keep;
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
        self.hovering_on_category.draw(g);
        self.panel.draw(g);
    }
}

fn build_panel(
    ctx: &mut EventCtx,
    app: &App,
    amenities: &Vec<AmenityType>,
    amenities_reachable: &MultiMap<AmenityType, BuildingID>,
    matches_all: usize,
) -> Panel {
    let contents = vec![
        "What homes are within 15 minutes away?".text_widget(ctx),
        "Containing at least 1 of each:".text_widget(ctx),
        Widget::custom_row(
            amenities_reachable
                .borrow()
                .iter()
                .map(|(amenity, buildings)| {
                    ctx.style()
                        .btn_outline
                        .text(format!("{}: {}", amenity, buildings.len()))
                        .build_widget(ctx, format!("businesses: {}", amenity))
                        .margin_right(4)
                        .margin_below(4)
                })
                .collect(),
        )
        .flex_wrap(ctx, Percent::int(30)),
        format!(
            "{} houses match all categories",
            prettyprint_usize(matches_all)
        )
        .text_widget(ctx),
        Line("Darker is better; more categories")
            .secondary()
            .into_widget(ctx),
        ColorLegend::gradient_with_width(
            ctx,
            &app.cs.good_to_bad_red,
            vec!["0", &amenities.len().to_string()],
            150.0,
        ),
        ctx.style()
            .btn_outline
            .text("change scoring criteria")
            .build_def(ctx),
    ];

    common::build_panel(ctx, app, common::Mode::ScoreHomes, Widget::col(contents))
}
