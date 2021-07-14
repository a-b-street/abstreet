use geom::{Distance, Duration};
use map_model::PathConstraints;
use sim::{TripEndpoint, TripID, TripMode};
use widgetry::table::{Col, Filter, Table};
use widgetry::{EventCtx, Filler, GeomBatch, GfxCtx, Line, Outcome, Panel, State, Text, Widget};

use crate::app::{App, Transition};
use crate::sandbox::dashboards::generic_trip_table::{open_trip_transition, preview_trip};
use crate::sandbox::dashboards::DashTab;

pub struct ModeShift {
    tab: DashTab,
    table: Table<App, Entry, Filters>,
    panel: Panel,
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
        preview_trip(g, app, &self.panel, GeomBatch::new());
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

// TODO Sliders for ranges of time/distance/steepness
struct Filters;

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
                        estimated_driving_time: driving_path.estimate_duration(
                            map,
                            PathConstraints::Car,
                            None,
                        ),
                        estimated_biking_time: biking_path.estimate_duration(
                            map,
                            PathConstraints::Bike,
                            Some(map_model::MAX_BIKE_SPEED),
                        ),
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
        state: Filters,
        to_controls: Box::new(|_, _, _| Widget::nothing()),
        from_controls: Box::new(|_| Filters),
        apply: Box::new(|_, _, _| true),
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
