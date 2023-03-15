use abstutil::prettyprint_usize;
use map_gui::tools::draw_isochrone;
use map_gui::ID;
use map_model::AmenityType;
use widgetry::tools::{ChooseSomething, ColorLegend, URLManager};
use widgetry::{
    Cached, Choice, Color, Drawable, EventCtx, GeomBatch, GfxCtx, Line, Outcome, Panel, State,
    Text, Transition, Widget,
};

use crate::common::{HoverKey, HoverOnBuilding};
use crate::isochrone::{BorderIsochrone, Isochrone, Options};
use crate::{common, render, App};

// It could be useful in the future, but it's kind of noisy right now
const SHOW_BORDER_ISOCHRONE: bool = false;

pub struct FromAmenity {
    panel: Panel,
    draw_unwalkable_roads: Drawable,

    amenity_type: AmenityType,
    draw: Drawable,
    isochrone: Isochrone,
    hovering_on_bldg: Cached<HoverKey, HoverOnBuilding>,
}

impl FromAmenity {
    pub fn random_amenity(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
        Self::new_state(ctx, app, AmenityType::Cafe)
    }

    pub fn new_state(
        ctx: &mut EventCtx,
        app: &App,
        amenity_type: AmenityType,
    ) -> Box<dyn State<App>> {
        map_gui::tools::update_url_map_name(app);

        let draw_unwalkable_roads = render::draw_unwalkable_roads(ctx, app);

        // For a category, find all matching stores
        let mut stores = Vec::new();
        for b in app.map.all_buildings() {
            if b.has_amenity(amenity_type) {
                stores.push(b.id);
            }
        }
        let isochrone = Isochrone::new(ctx, app, stores, app.session.clone());

        let mut batch = GeomBatch::new();

        if SHOW_BORDER_ISOCHRONE {
            // Draw an isochrone showing the map boundary
            let mut borders = Vec::new();
            for i in app.map.all_intersections() {
                if i.is_border() {
                    borders.push(i.id);
                }
            }
            let border_isochrone = BorderIsochrone::new(ctx, app, borders, app.session.clone());

            batch.append(draw_isochrone(
                &app.map,
                &border_isochrone.time_to_reach_building,
                &border_isochrone.thresholds,
                &border_isochrone.colors,
            ));
        }

        batch.append(draw_isochrone(
            &app.map,
            &isochrone.time_to_reach_building,
            &isochrone.thresholds,
            &isochrone.colors,
        ));
        for &start in &isochrone.start {
            batch.append(render::draw_star(ctx, app.map.get_b(start)));
        }

        let panel = build_panel(ctx, app, amenity_type, &isochrone);

        Box::new(Self {
            panel,
            draw_unwalkable_roads,

            amenity_type,
            draw: ctx.upload(batch),
            isochrone,
            hovering_on_bldg: Cached::new(),
        })
    }
}

impl State<App> for FromAmenity {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition<App> {
        // Allow panning and zooming
        if ctx.canvas_movement() {
            URLManager::update_url_cam(ctx, app.map.get_gps_bounds());
        }

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

        match self.panel.event(ctx) {
            Outcome::Clicked(x) => {
                if x == "explore matching amenities" {
                    return Transition::Push(
                        crate::amenities_details::ExploreAmenitiesDetails::new_state(
                            ctx,
                            app,
                            &self.isochrone,
                            self.amenity_type,
                        ),
                    );
                } else if x == "change amenity type" {
                    return Transition::Push(ChooseSomething::new_state(
                        ctx,
                        "Search from all amenities of what type?",
                        app.map
                            .get_available_amenity_types()
                            .into_iter()
                            .map(|at| Choice::new(at.to_string(), at))
                            .collect(),
                        Box::new(move |choice, ctx, app| {
                            Transition::Multi(vec![
                                Transition::Pop,
                                Transition::Replace(Self::new_state(ctx, app, choice)),
                            ])
                        }),
                    ));
                }

                return common::on_click(ctx, app, &x);
            }
            Outcome::Changed(_) => {
                app.session = Options {
                    movement: common::options_from_controls(&self.panel),
                    thresholds: Options::default_thresholds(),
                };
                return Transition::Replace(Self::new_state(ctx, app, self.amenity_type));
            }
            _ => {}
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        g.redraw(&self.draw);
        g.redraw(&self.draw_unwalkable_roads);
        self.panel.draw(g);
        if let Some(hover) = self.hovering_on_bldg.value() {
            g.draw_mouse_tooltip(hover.tooltip.clone());
            g.redraw(&hover.drawn_route);
        }
    }
}

fn build_panel(
    ctx: &mut EventCtx,
    app: &App,
    amenity_type: AmenityType,
    isochrone: &Isochrone,
) -> Panel {
    let contents = vec![
        Line(format!("What's within 15 minutes of all {}", amenity_type)).into_widget(ctx),
        Widget::row(vec![
            Line("Change amenity type:").into_widget(ctx),
            ctx.style()
                .btn_outline
                .text(amenity_type.to_string())
                .build_widget(ctx, "change amenity type"),
        ]),
        ctx.style()
            .btn_outline
            .text(format!("{} matching amenities", isochrone.start.len()))
            .build_widget(ctx, "explore matching amenities"),
        Text::from_all(vec![
            Line("Estimated population: ").secondary(),
            Line(prettyprint_usize(isochrone.population)),
        ])
        .into_widget(ctx),
        Text::from_all(vec![
            Line("Estimated street parking spots: ").secondary(),
            Line(prettyprint_usize(isochrone.onstreet_parking_spots)),
        ])
        .into_widget(ctx),
        ColorLegend::categories(
            ctx,
            vec![
                (Color::GREEN, "0 mins"),
                (Color::ORANGE, "5"),
                (Color::RED, "10"),
            ],
            "15",
        ),
        if SHOW_BORDER_ISOCHRONE {
            ColorLegend::row(
                ctx,
                Color::rgb(0, 0, 0).alpha(0.3),
                "< 15 mins from border (amenity could exist off map)",
            )
        } else {
            Widget::nothing()
        },
    ];

    common::build_panel(
        ctx,
        app,
        common::Mode::StartFromAmenity,
        Widget::col(contents),
    )
}
