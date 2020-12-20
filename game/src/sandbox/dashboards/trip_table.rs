use std::collections::{BTreeSet, HashMap};

use abstutil::prettyprint_usize;
use geom::{Duration, Time};
use sim::{TripEndpoint, TripID, TripMode};
use widgetry::table::{Col, Filter, Table};
use widgetry::{Btn, Checkbox, EventCtx, Filler, Line, Panel, State, Text, Widget};

use crate::app::App;
use crate::common::{checkbox_per_mode, cmp_duration_shorter, color_for_mode};
use crate::sandbox::dashboards::generic_trip_table::GenericTripTable;
use crate::sandbox::dashboards::DashTab;

pub struct FinishedTripTable;

impl FinishedTripTable {
    pub fn new(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
        GenericTripTable::new(
            ctx,
            app,
            DashTab::FinishedTripTable,
            make_table_finished_trips(app),
            make_panel_finished_trips,
        )
    }
}

pub struct CancelledTripTable;

impl CancelledTripTable {
    pub fn new(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
        GenericTripTable::new(
            ctx,
            app,
            DashTab::CancelledTripTable,
            make_table_cancelled_trips(app),
            make_panel_cancelled_trips,
        )
    }
}

pub struct UnfinishedTripTable;

impl UnfinishedTripTable {
    pub fn new(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
        GenericTripTable::new(
            ctx,
            app,
            DashTab::UnfinishedTripTable,
            make_table_unfinished_trips(app),
            make_panel_unfinished_trips,
        )
    }
}

struct FinishedTrip {
    id: TripID,
    mode: TripMode,
    modified: bool,
    capped: bool,
    starts_off_map: bool,
    ends_off_map: bool,
    departure: Time,
    duration_after: Duration,
    duration_before: Duration,
    waiting: Duration,
    percent_waiting: usize,
}

struct CancelledTrip {
    id: TripID,
    mode: TripMode,
    departure: Time,
    starts_off_map: bool,
    ends_off_map: bool,
    duration_before: Duration,
    reason: String,
}

struct UnfinishedTrip {
    id: TripID,
    mode: TripMode,
    departure: Time,
    duration_before: Duration,
    // TODO Estimated wait time?
}

struct Filters {
    modes: BTreeSet<TripMode>,
    off_map_starts: bool,
    off_map_ends: bool,
    unmodified_trips: bool,
    modified_trips: bool,
    uncapped_trips: bool,
    capped_trips: bool,
}

fn produce_raw_data(app: &App) -> (Vec<FinishedTrip>, Vec<CancelledTrip>) {
    let mut finished = Vec::new();
    let mut cancelled = Vec::new();

    // Only make one pass through prebaked data
    let trip_times_before = if app.has_prebaked().is_some() {
        let mut times = HashMap::new();
        for (_, id, _, maybe_dt) in &app.prebaked().finished_trips {
            if let Some(dt) = maybe_dt {
                times.insert(*id, *dt);
            }
        }
        Some(times)
    } else {
        None
    };

    let sim = &app.primary.sim;
    for (_, id, mode, maybe_duration_after) in &sim.get_analytics().finished_trips {
        let trip = sim.trip_info(*id);
        let starts_off_map = match trip.start {
            TripEndpoint::Border(_) => true,
            _ => false,
        };
        let ends_off_map = match trip.end {
            TripEndpoint::Border(_) => true,
            _ => false,
        };
        let duration_before = if let Some(ref times) = trip_times_before {
            times.get(id).cloned()
        } else {
            Some(Duration::ZERO)
        };

        if maybe_duration_after.is_none() || duration_before.is_none() {
            let reason = trip.cancellation_reason.clone().unwrap_or(format!(
                "trip succeeded now, but not before the current proposal"
            ));
            cancelled.push(CancelledTrip {
                id: *id,
                mode: *mode,
                departure: trip.departure,
                starts_off_map,
                ends_off_map,
                duration_before: duration_before.unwrap_or(Duration::ZERO),
                reason,
            });
            continue;
        };

        let (_, waiting, _) = sim.finished_trip_details(*id).unwrap();

        let duration_after = maybe_duration_after.unwrap();
        finished.push(FinishedTrip {
            id: *id,
            mode: *mode,
            departure: trip.departure,
            modified: trip.modified,
            capped: trip.capped,
            starts_off_map,
            ends_off_map,
            duration_after,
            duration_before: duration_before.unwrap(),
            waiting,
            percent_waiting: (100.0 * waiting / duration_after) as usize,
        });
    }

    (finished, cancelled)
}

fn make_table_finished_trips(app: &App) -> Table<App, FinishedTrip, Filters> {
    let (finished, _) = produce_raw_data(app);
    let any_congestion_caps = app
        .primary
        .map
        .all_zones()
        .iter()
        .any(|z| z.restrictions.cap_vehicles_per_hour.is_some());
    let filter: Filter<App, FinishedTrip, Filters> = Filter {
        state: Filters {
            modes: TripMode::all().into_iter().collect(),
            off_map_starts: true,
            off_map_ends: true,
            unmodified_trips: true,
            modified_trips: true,
            uncapped_trips: true,
            capped_trips: true,
        },
        to_controls: Box::new(move |ctx, app, state| {
            Widget::col(vec![
                checkbox_per_mode(ctx, app, &state.modes),
                Widget::row(vec![
                    Checkbox::switch(ctx, "starting off-map", None, state.off_map_starts),
                    Checkbox::switch(ctx, "ending off-map", None, state.off_map_ends),
                    if app.primary.has_modified_trips {
                        Checkbox::switch(
                            ctx,
                            "trips unmodified by experiment",
                            None,
                            state.unmodified_trips,
                        )
                    } else {
                        Widget::nothing()
                    },
                    if app.primary.has_modified_trips {
                        Checkbox::switch(
                            ctx,
                            "trips modified by experiment",
                            None,
                            state.modified_trips,
                        )
                    } else {
                        Widget::nothing()
                    },
                    if any_congestion_caps {
                        Checkbox::switch(
                            ctx,
                            "trips not affected by congestion caps",
                            None,
                            state.uncapped_trips,
                        )
                    } else {
                        Widget::nothing()
                    },
                    if any_congestion_caps {
                        Checkbox::switch(
                            ctx,
                            "trips affected by congestion caps",
                            None,
                            state.capped_trips,
                        )
                    } else {
                        Widget::nothing()
                    },
                ]),
            ])
        }),
        from_controls: Box::new(|panel| {
            let mut modes = BTreeSet::new();
            for m in TripMode::all() {
                if panel.is_checked(m.ongoing_verb()) {
                    modes.insert(m);
                }
            }
            Filters {
                modes,
                off_map_starts: panel.is_checked("starting off-map"),
                off_map_ends: panel.is_checked("ending off-map"),
                unmodified_trips: panel
                    .maybe_is_checked("trips unmodified by experiment")
                    .unwrap_or(true),
                modified_trips: panel
                    .maybe_is_checked("trips modified by experiment")
                    .unwrap_or(true),
                uncapped_trips: panel
                    .maybe_is_checked("trips not affected by congestion caps")
                    .unwrap_or(true),
                capped_trips: panel
                    .maybe_is_checked("trips affected by congestion caps")
                    .unwrap_or(true),
            }
        }),
        apply: Box::new(|state, x| {
            if !state.modes.contains(&x.mode) {
                return false;
            }
            if !state.off_map_starts && x.starts_off_map {
                return false;
            }
            if !state.off_map_ends && x.ends_off_map {
                return false;
            }
            if !state.unmodified_trips && !x.modified {
                return false;
            }
            if !state.modified_trips && x.modified {
                return false;
            }
            if !state.uncapped_trips && !x.capped {
                return false;
            }
            if !state.capped_trips && x.capped {
                return false;
            }
            true
        }),
    };

    let mut table = Table::new(
        finished,
        Box::new(|x| x.id.0.to_string()),
        "Percent waiting",
        filter,
    );
    table.static_col("Trip ID", Box::new(|x| x.id.0.to_string()));
    if app.primary.has_modified_trips {
        table.static_col(
            "Modified",
            Box::new(|x| {
                if x.modified {
                    "Yes".to_string()
                } else {
                    "No".to_string()
                }
            }),
        );
    }
    if any_congestion_caps {
        table.static_col(
            "Capped",
            Box::new(|x| {
                if x.capped {
                    "Yes".to_string()
                } else {
                    "No".to_string()
                }
            }),
        );
    }
    table.column(
        "Type",
        Box::new(|ctx, app, x| {
            Text::from(Line(x.mode.ongoing_verb()).fg(color_for_mode(app, x.mode))).render(ctx)
        }),
        Col::Static,
    );
    table.column(
        "Departure",
        Box::new(|ctx, _, x| Text::from(Line(x.departure.ampm_tostring())).render(ctx)),
        Col::Sortable(Box::new(|rows| rows.sort_by_key(|x| x.departure))),
    );
    table.column(
        "Duration",
        Box::new(|ctx, app, x| {
            Text::from(Line(x.duration_after.to_string(&app.opts.units))).render(ctx)
        }),
        Col::Sortable(Box::new(|rows| rows.sort_by_key(|x| x.duration_after))),
    );

    if app.has_prebaked().is_some() {
        table.column(
            "Comparison",
            Box::new(|ctx, app, x| {
                Text::from_all(cmp_duration_shorter(
                    app,
                    x.duration_after,
                    x.duration_before,
                ))
                .render(ctx)
            }),
            Col::Sortable(Box::new(|rows| {
                rows.sort_by_key(|x| x.duration_after - x.duration_before)
            })),
        );
        table.column(
            "Normalized",
            Box::new(|ctx, _, x| {
                Text::from(Line(if x.duration_after == x.duration_before {
                    format!("same")
                } else if x.duration_after < x.duration_before {
                    format!(
                        "{}% faster",
                        (100.0 * (1.0 - (x.duration_after / x.duration_before))) as usize
                    )
                } else {
                    format!(
                        "{}% slower ",
                        (100.0 * ((x.duration_after / x.duration_before) - 1.0)) as usize
                    )
                }))
                .render(ctx)
            }),
            Col::Sortable(Box::new(|rows| {
                rows.sort_by_key(|x| (100.0 * (x.duration_after / x.duration_before)) as isize)
            })),
        );
    }

    table.column(
        "Time spent waiting",
        Box::new(|ctx, app, x| Text::from(Line(x.waiting.to_string(&app.opts.units))).render(ctx)),
        Col::Sortable(Box::new(|rows| rows.sort_by_key(|x| x.waiting))),
    );
    table.column(
        "Percent waiting",
        Box::new(|ctx, _, x| Text::from(Line(x.percent_waiting.to_string())).render(ctx)),
        Col::Sortable(Box::new(|rows| rows.sort_by_key(|x| x.percent_waiting))),
    );

    table
}

fn make_table_cancelled_trips(app: &App) -> Table<App, CancelledTrip, Filters> {
    let (_, cancelled) = produce_raw_data(app);
    // Reuse the same filters, but ignore modified and capped trips
    let filter: Filter<App, CancelledTrip, Filters> = Filter {
        state: Filters {
            modes: TripMode::all().into_iter().collect(),
            off_map_starts: true,
            off_map_ends: true,
            unmodified_trips: true,
            modified_trips: true,
            uncapped_trips: true,
            capped_trips: true,
        },
        to_controls: Box::new(move |ctx, app, state| {
            Widget::col(vec![
                checkbox_per_mode(ctx, app, &state.modes),
                Widget::row(vec![
                    Checkbox::switch(ctx, "starting off-map", None, state.off_map_starts),
                    Checkbox::switch(ctx, "ending off-map", None, state.off_map_ends),
                ]),
            ])
        }),
        from_controls: Box::new(|panel| {
            let mut modes = BTreeSet::new();
            for m in TripMode::all() {
                if panel.is_checked(m.ongoing_verb()) {
                    modes.insert(m);
                }
            }
            Filters {
                modes,
                off_map_starts: panel.is_checked("starting off-map"),
                off_map_ends: panel.is_checked("ending off-map"),
                unmodified_trips: true,
                modified_trips: true,
                uncapped_trips: true,
                capped_trips: true,
            }
        }),
        apply: Box::new(|state, x| {
            if !state.modes.contains(&x.mode) {
                return false;
            }
            if !state.off_map_starts && x.starts_off_map {
                return false;
            }
            if !state.off_map_ends && x.ends_off_map {
                return false;
            }
            true
        }),
    };

    let mut table = Table::new(
        cancelled,
        Box::new(|x| x.id.0.to_string()),
        "Departure",
        filter,
    );
    table.static_col("Trip ID", Box::new(|x| x.id.0.to_string()));
    table.column(
        "Type",
        Box::new(|ctx, app, x| {
            Text::from(Line(x.mode.ongoing_verb()).fg(color_for_mode(app, x.mode))).render(ctx)
        }),
        Col::Static,
    );
    table.column(
        "Departure",
        Box::new(|ctx, _, x| Text::from(Line(x.departure.ampm_tostring())).render(ctx)),
        Col::Sortable(Box::new(|rows| rows.sort_by_key(|x| x.departure))),
    );
    if app.has_prebaked().is_some() {
        table.column(
            "Estimated duration",
            Box::new(|ctx, app, x| {
                Text::from(Line(x.duration_before.to_string(&app.opts.units))).render(ctx)
            }),
            Col::Sortable(Box::new(|rows| rows.sort_by_key(|x| x.duration_before))),
        );
    }
    table.static_col("Reason", Box::new(|x| x.reason.clone()));

    table
}

fn make_table_unfinished_trips(app: &App) -> Table<App, UnfinishedTrip, Filters> {
    // Only make one pass through prebaked data
    let trip_times_before = if app.has_prebaked().is_some() {
        let mut times = HashMap::new();
        for (_, id, _, maybe_dt) in &app.prebaked().finished_trips {
            if let Some(dt) = maybe_dt {
                times.insert(*id, *dt);
            }
        }
        Some(times)
    } else {
        None
    };
    let mut unfinished = Vec::new();
    for (id, trip) in app.primary.sim.all_trip_info() {
        if app.primary.sim.finished_trip_details(id).is_none() {
            let duration_before = trip_times_before
                .as_ref()
                .and_then(|times| times.get(&id))
                .cloned()
                .unwrap_or(Duration::ZERO);
            unfinished.push(UnfinishedTrip {
                id,
                mode: trip.mode,
                departure: trip.departure,
                duration_before,
            });
        }
    }

    // Reuse the same filters, but ignore modified and capped trips
    let filter: Filter<App, UnfinishedTrip, Filters> = Filter {
        state: Filters {
            modes: TripMode::all().into_iter().collect(),
            off_map_starts: true,
            off_map_ends: true,
            unmodified_trips: true,
            modified_trips: true,
            uncapped_trips: true,
            capped_trips: true,
        },
        to_controls: Box::new(move |ctx, app, state| checkbox_per_mode(ctx, app, &state.modes)),
        from_controls: Box::new(|panel| {
            let mut modes = BTreeSet::new();
            for m in TripMode::all() {
                if panel.is_checked(m.ongoing_verb()) {
                    modes.insert(m);
                }
            }
            Filters {
                modes,
                off_map_starts: true,
                off_map_ends: true,
                unmodified_trips: true,
                modified_trips: true,
                uncapped_trips: true,
                capped_trips: true,
            }
        }),
        apply: Box::new(|state, x| {
            if !state.modes.contains(&x.mode) {
                return false;
            }
            true
        }),
    };

    let mut table = Table::new(
        unfinished,
        Box::new(|x| x.id.0.to_string()),
        "Departure",
        filter,
    );
    table.static_col("Trip ID", Box::new(|x| x.id.0.to_string()));
    table.column(
        "Type",
        Box::new(|ctx, app, x| {
            Text::from(Line(x.mode.ongoing_verb()).fg(color_for_mode(app, x.mode))).render(ctx)
        }),
        Col::Static,
    );
    table.column(
        "Departure",
        Box::new(|ctx, _, x| Text::from(Line(x.departure.ampm_tostring())).render(ctx)),
        Col::Sortable(Box::new(|rows| rows.sort_by_key(|x| x.departure))),
    );
    if app.has_prebaked().is_some() {
        table.column(
            "Estimated duration",
            Box::new(|ctx, app, x| {
                Text::from(Line(x.duration_before.to_string(&app.opts.units))).render(ctx)
            }),
            Col::Sortable(Box::new(|rows| rows.sort_by_key(|x| x.duration_before))),
        );
    }

    table
}

fn trip_category_selector(ctx: &mut EventCtx, app: &App, tab: DashTab) -> Widget {
    let (finished, unfinished) = app.primary.sim.num_trips();
    let mut cancelled = 0;
    // TODO Can we avoid iterating through this again?
    for (_, _, _, maybe_dt) in &app.primary.sim.get_analytics().finished_trips {
        if maybe_dt.is_none() {
            cancelled += 1;
        }
    }
    let total = finished + cancelled + unfinished;

    let btn = |dash, action, label| {
        if dash == tab {
            Text::from(Line(label).underlined())
                .draw(ctx)
                .centered_vert()
        } else {
            Btn::plaintext(label).build(ctx, action, None)
        }
    };

    Widget::custom_row(vec![
        btn(
            DashTab::FinishedTripTable,
            "finished trips",
            format!(
                "{} ({:.1}%) Finished Trips",
                prettyprint_usize(finished),
                if total > 0 {
                    (finished as f64) / (total as f64) * 100.0
                } else {
                    0.0
                }
            ),
        )
        .margin_right(28),
        btn(
            DashTab::CancelledTripTable,
            "cancelled trips",
            format!("{} Cancelled Trips", prettyprint_usize(cancelled)),
        )
        .margin_right(28),
        btn(
            DashTab::UnfinishedTripTable,
            "unfinished trips",
            format!(
                "{} ({:.1}%) Unfinished Trips",
                prettyprint_usize(unfinished),
                if total > 0 {
                    (unfinished as f64) / (total as f64) * 100.0
                } else {
                    0.0
                }
            ),
        ),
    ])
}

fn make_panel_finished_trips(
    ctx: &mut EventCtx,
    app: &App,
    table: &Table<App, FinishedTrip, Filters>,
) -> Panel {
    Panel::new(Widget::col(vec![
        DashTab::FinishedTripTable.picker(ctx, app),
        trip_category_selector(ctx, app, DashTab::FinishedTripTable),
        table.render(ctx, app),
        Filler::square_width(ctx, 0.15)
            .named("preview")
            .centered_horiz(),
    ]))
    .exact_size_percent(90, 90)
    .build(ctx)
}

// Always use DashTab::FinishedTripTable, so the dropdown works
fn make_panel_cancelled_trips(
    ctx: &mut EventCtx,
    app: &App,
    table: &Table<App, CancelledTrip, Filters>,
) -> Panel {
    Panel::new(Widget::col(vec![
        DashTab::FinishedTripTable.picker(ctx, app),
        trip_category_selector(ctx, app, DashTab::CancelledTripTable),
        table.render(ctx, app),
        Filler::square_width(ctx, 0.15)
            .named("preview")
            .centered_horiz(),
    ]))
    .exact_size_percent(90, 90)
    .build(ctx)
}

fn make_panel_unfinished_trips(
    ctx: &mut EventCtx,
    app: &App,
    table: &Table<App, UnfinishedTrip, Filters>,
) -> Panel {
    Panel::new(Widget::col(vec![
        DashTab::FinishedTripTable.picker(ctx, app),
        trip_category_selector(ctx, app, DashTab::UnfinishedTripTable),
        table.render(ctx, app),
        Filler::square_width(ctx, 0.15)
            .named("preview")
            .centered_horiz(),
    ]))
    .exact_size_percent(90, 90)
    .build(ctx)
}
