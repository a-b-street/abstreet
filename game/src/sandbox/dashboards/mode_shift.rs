use std::collections::HashSet;

use abstutil::Counter;
use geom::{Distance, Duration};
use map_gui::tools::ColorNetwork;
use map_model::{PathStepV2, RoadID};
use sim::{TripEndpoint, TripID, TripMode};
use widgetry::table::{Col, Filter, Table};
use widgetry::{
    Drawable, EventCtx, Filler, GeomBatch, GfxCtx, Line, Outcome, Panel, Spinner, State, Text,
    TextExt, Widget,
};

use crate::app::{App, Transition};
use crate::sandbox::dashboards::generic_trip_table::{open_trip_transition, preview_trip};
use crate::sandbox::dashboards::DashTab;

pub struct ModeShift {
    tab: DashTab,
    table: Table<App, Entry, Filters>,
    panel: Panel,
    show_route_gaps: Drawable,
}

impl ModeShift {
    pub fn new_state(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
        let table = make_table(ctx, app);
        let col = Widget::col(vec![
            DashTab::ModeShift.picker(ctx, app),
            Widget::col(vec![
                Text::from_multiline(vec![
                    Line("This looks at transforming driving trips into cycling."),
                    Line("Off-map starts/ends are excluded."),
                ])
                .into_widget(ctx),
                ctx.style()
                    .btn_outline
                    .text("Show most important gaps in cycling infrastructure")
                    .build_def(ctx),
                table.render(ctx, app),
                Filler::square_width(ctx, 0.15).named("preview"),
            ])
            .section(ctx),
        ]);

        let panel = Panel::new_builder(col)
            .exact_size_percent(90, 90)
            .build(ctx);

        Box::new(Self {
            tab: DashTab::ModeShift,
            table,
            panel,
            show_route_gaps: Drawable::empty(ctx),
        })
    }
}

impl State<App> for ModeShift {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => {
                if self.table.clicked(&x) {
                    self.table.replace_render(ctx, app, &mut self.panel);
                } else if let Ok(idx) = x.parse::<usize>() {
                    return open_trip_transition(app, idx);
                } else if x == "close" {
                    return Transition::Pop;
                } else if x == "Show most important gaps in cycling infrastructure" {
                    // TODO Automatically recalculate as filters change? Too slow.
                    self.show_route_gaps = show_route_gaps(ctx, app, &self.table);
                } else {
                    unreachable!()
                }
            }
            Outcome::Changed(_) => {
                if let Some(t) = self.tab.transition(ctx, app, &self.panel) {
                    return t;
                }

                self.table.panel_changed(&self.panel);
                self.table.replace_render(ctx, app, &mut self.panel);
            }
            _ => {}
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.panel.draw(g);
        // TODO This only draws a route if the trip has already happened in the simulation
        preview_trip(
            g,
            app,
            &self.panel,
            GeomBatch::new(),
            Some(&self.show_route_gaps),
        );
    }
}

struct Entry {
    trip: TripID,
    estimated_driving_time: Duration,
    // Only when we prebaked data?
    //actual_driving_time: Duration,
    estimated_biking_time: Duration,
    distance: Distance,
    total_elevation_gain: Distance,
    total_elevation_loss: Distance,
}

struct Filters {
    max_driving_time: Duration,
    max_biking_time: Duration,
    max_distance: Distance,
    max_elevation_gain: Distance,
}

fn produce_raw_data(ctx: &mut EventCtx, app: &App) -> Vec<Entry> {
    let map = &app.primary.map;
    ctx.loading_screen("shift modes", |_, timer| {
        timer.parallelize(
            "analyze trips",
            app.primary
                .sim
                .all_trip_info()
                .into_iter()
                .filter_map(|(id, info)| {
                    if info.mode == TripMode::Drive
                        && matches!(info.start, TripEndpoint::Bldg(_))
                        && matches!(info.end, TripEndpoint::Bldg(_))
                    {
                        Some((id, info))
                    } else {
                        None
                    }
                })
                .collect(),
            |(id, info)| {
                // TODO Does ? work
                if let (Some(driving_path), Some(biking_path)) = (
                    TripEndpoint::path_req(info.start, info.end, TripMode::Drive, map)
                        .and_then(|req| map.pathfind(req).ok()),
                    TripEndpoint::path_req(info.start, info.end, TripMode::Bike, map)
                        .and_then(|req| map.pathfind(req).ok()),
                ) {
                    let (total_elevation_gain, total_elevation_loss) =
                        biking_path.get_total_elevation_change(map);
                    Some(Entry {
                        trip: id,
                        estimated_driving_time: driving_path.estimate_duration(map, None),
                        estimated_biking_time: biking_path
                            .estimate_duration(map, Some(map_model::MAX_BIKE_SPEED)),
                        // TODO The distance (and elevation change) might differ between the two
                        // paths if there's a highway or a trail. For now, just use the biking
                        // distance.
                        distance: biking_path.total_length(),
                        total_elevation_gain,
                        total_elevation_loss,
                    })
                } else {
                    None
                }
            },
        )
    })
    .into_iter()
    .flatten()
    .collect()
}

fn make_table(ctx: &mut EventCtx, app: &App) -> Table<App, Entry, Filters> {
    let filter: Filter<App, Entry, Filters> = Filter {
        state: Filters {
            // Just some sample defaults
            max_driving_time: Duration::minutes(30),
            max_biking_time: Duration::minutes(30),
            max_distance: Distance::miles(10.0),
            max_elevation_gain: Distance::feet(30.0),
        },
        to_controls: Box::new(|ctx, _, state| {
            Widget::row(vec![
                Widget::row(vec![
                    "Max driving time".text_widget(ctx).centered_vert(),
                    Spinner::widget(
                        ctx,
                        "max_driving_time",
                        (Duration::ZERO, Duration::hours(12)),
                        state.max_driving_time,
                        Duration::minutes(1),
                    ),
                ]),
                Widget::row(vec![
                    "Max biking time".text_widget(ctx).centered_vert(),
                    Spinner::widget(
                        ctx,
                        "max_biking_time",
                        (Duration::ZERO, Duration::hours(12)),
                        state.max_biking_time,
                        Duration::minutes(1),
                    ),
                ]),
                Widget::row(vec![
                    "Max distance".text_widget(ctx).centered_vert(),
                    Spinner::widget(
                        ctx,
                        "max_distance",
                        (Distance::ZERO, Distance::miles(20.0)),
                        state.max_distance,
                        Distance::miles(0.1),
                    ),
                ]),
                Widget::row(vec![
                    "Max elevation gain".text_widget(ctx).centered_vert(),
                    Spinner::widget(
                        ctx,
                        "max_elevation_gain",
                        (Distance::ZERO, Distance::feet(500.0)),
                        state.max_elevation_gain,
                        Distance::feet(10.0),
                    ),
                ]),
            ])
            .evenly_spaced()
        }),
        from_controls: Box::new(|panel| Filters {
            max_driving_time: panel.spinner("max_driving_time"),
            max_biking_time: panel.spinner("max_biking_time"),
            max_distance: panel.spinner("max_distance"),
            max_elevation_gain: panel.spinner("max_elevation_gain"),
        }),
        apply: Box::new(|state, x, _| {
            x.estimated_driving_time <= state.max_driving_time
                && x.estimated_biking_time <= state.max_biking_time
                && x.distance <= state.max_distance
                && x.total_elevation_gain <= state.max_elevation_gain
        }),
    };

    let mut table = Table::new(
        "mode_shift",
        produce_raw_data(ctx, app),
        Box::new(|x| x.trip.0.to_string()),
        "Estimated driving time",
        filter,
    );
    table.static_col("Trip ID", Box::new(|x| x.trip.0.to_string()));
    table.column(
        "Estimated driving time",
        Box::new(|ctx, app, x| {
            Text::from(x.estimated_driving_time.to_string(&app.opts.units)).render(ctx)
        }),
        Col::Sortable(Box::new(|rows| {
            rows.sort_by_key(|x| x.estimated_driving_time)
        })),
    );
    table.column(
        "Estimated biking time",
        Box::new(|ctx, app, x| {
            Text::from(x.estimated_biking_time.to_string(&app.opts.units)).render(ctx)
        }),
        Col::Sortable(Box::new(|rows| {
            rows.sort_by_key(|x| x.estimated_biking_time)
        })),
    );
    table.column(
        "Distance",
        Box::new(|ctx, app, x| Text::from(x.distance.to_string(&app.opts.units)).render(ctx)),
        Col::Sortable(Box::new(|rows| rows.sort_by_key(|x| x.distance))),
    );
    table.column(
        "Elevation gain/loss",
        Box::new(|ctx, app, x| {
            Text::from(format!(
                "Up {}, down {}",
                x.total_elevation_gain.to_string(&app.opts.units),
                x.total_elevation_loss.to_string(&app.opts.units)
            ))
            .render(ctx)
        }),
        // Maybe some kind of sorting / filtering actually would be useful here
        Col::Static,
    );

    table
}

fn show_route_gaps(ctx: &mut EventCtx, app: &App, table: &Table<App, Entry, Filters>) -> Drawable {
    ctx.loading_screen("calculate all routes", |ctx, timer| {
        let map = &app.primary.map;
        let sim = &app.primary.sim;

        // Find all high-stress roads, since we'll filter by them next
        let high_stress: HashSet<RoadID> = map
            .all_roads()
            .iter()
            .filter_map(|r| {
                if r.high_stress_for_bikes(map) {
                    Some(r.id)
                } else {
                    None
                }
            })
            .collect();

        let mut road_counter = Counter::new();
        for path in timer
            .parallelize("calculate routes", table.get_filtered_data(app), |entry| {
                let info = sim.trip_info(entry.trip);
                TripEndpoint::path_req(info.start, info.end, TripMode::Bike, map)
                    .and_then(|req| map.pathfind_v2(req).ok())
            })
            .into_iter()
            .flatten()
        {
            for step in path.get_steps() {
                // No Contraflow steps for bike paths
                if let PathStepV2::Along(dr) = step {
                    if high_stress.contains(&dr.id) {
                        road_counter.inc(dr.id);
                    }
                }
            }
        }

        let mut colorer = ColorNetwork::new(app);
        colorer.ranked_roads(road_counter, &app.cs.good_to_bad_red);
        colorer.build(ctx).0
    })
}
