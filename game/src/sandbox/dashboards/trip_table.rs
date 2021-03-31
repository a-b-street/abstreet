use std::collections::{BTreeSet, HashMap};

use abstutil::prettyprint_usize;
use geom::{Duration, Polygon, Time};
use sim::{TripEndpoint, TripID, TripMode};
use widgetry::table::{Col, Filter, Table};
use widgetry::{
    EventCtx, Filler, GfxCtx, Line, Outcome, Panel, Stash, State, TabController, Text, Toggle,
    Widget,
};

use super::generic_trip_table::{open_trip_transition, preview_trip};
use super::selector::RectangularSelector;
use super::DashTab;
use crate::app::{App, Transition};
use crate::common::{checkbox_per_mode, cmp_duration_shorter, color_for_mode};

pub struct TripTable {
    tab: DashTab,
    table_tabs: TabController,
    panel: Panel,
    finished_trips_table: Table<App, FinishedTrip, Filters>,
    cancelled_trips_table: Table<App, CancelledTrip, Filters>,
    unfinished_trips_table: Table<App, UnfinishedTrip, Filters>,
    dirty: bool,
}

impl TripTable {
    pub fn new(ctx: &mut EventCtx, app: &App) -> Self {
        let mut tabs = TabController::new("trips_tabs");

        let (finished, unfinished) = app.primary.sim.num_trips();
        let mut cancelled = 0;
        // TODO Can we avoid iterating through this again?
        for (_, _, _, maybe_dt) in &app.primary.sim.get_analytics().finished_trips {
            if maybe_dt.is_none() {
                cancelled += 1;
            }
        }
        let total = finished + cancelled + unfinished;

        let percent = |x: usize| -> f64 {
            if total > 0 {
                (x as f64) / (total as f64) * 100.0
            } else {
                0.0
            }
        };

        let finished_trips_btn = ctx
            .style()
            .btn_tab
            .text(format!(
                "Finished Trips: {} ({:.1}%)",
                prettyprint_usize(finished),
                percent(finished),
            ))
            .tooltip("Finished Trips");

        let finished_trips_table = make_table_finished_trips(app);
        let finished_trips_content = Widget::col(vec![
            finished_trips_table.render(ctx, app),
            Filler::square_width(ctx, 0.15)
                .named("preview")
                .centered_horiz(),
        ]);
        tabs.push_tab(finished_trips_btn, finished_trips_content);

        let cancelled_trips_table = make_table_cancelled_trips(app);
        let cancelled_trips_btn = ctx
            .style()
            .btn_tab
            .text(format!("Cancelled Trips: {}", prettyprint_usize(cancelled)))
            .tooltip("Cancelled Trips");
        let cancelled_trips_content = Widget::col(vec![
            cancelled_trips_table.render(ctx, app),
            Filler::square_width(ctx, 0.15)
                .named("preview")
                .centered_horiz(),
        ]);
        tabs.push_tab(cancelled_trips_btn, cancelled_trips_content);

        let unfinished_trips_table = make_table_unfinished_trips(app);
        let unfinished_trips_btn = ctx
            .style()
            .btn_tab
            .text(format!(
                "Unfinished Trips: {} ({:.1}%)",
                prettyprint_usize(unfinished),
                percent(unfinished)
            ))
            .tooltip("Unfinished Trips");
        let unfinished_trips_content = Widget::col(vec![
            unfinished_trips_table.render(ctx, app),
            Filler::square_width(ctx, 0.15)
                .named("preview")
                .centered_horiz(),
        ]);
        tabs.push_tab(unfinished_trips_btn, unfinished_trips_content);

        let panel = Panel::new(Widget::col(vec![
            DashTab::TripTable.picker(ctx, app),
            tabs.build_widget(ctx),
        ]))
        .exact_size_percent(90, 90)
        .build(ctx);

        Self {
            tab: DashTab::TripTable,
            table_tabs: tabs,
            panel,
            finished_trips_table,
            cancelled_trips_table,
            unfinished_trips_table,
            dirty: false,
        }
    }
}

impl State<App> for TripTable {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => {
                if self.table_tabs.active_tab_idx() == 0 && self.finished_trips_table.clicked(&x) {
                    self.finished_trips_table
                        .replace_render(ctx, app, &mut self.panel);
                } else if self.table_tabs.active_tab_idx() == 1
                    && self.cancelled_trips_table.clicked(&x)
                {
                    self.cancelled_trips_table
                        .replace_render(ctx, app, &mut self.panel);
                } else if self.table_tabs.active_tab_idx() == 2
                    && self.unfinished_trips_table.clicked(&x)
                {
                    self.unfinished_trips_table
                        .replace_render(ctx, app, &mut self.panel);
                } else if let Ok(idx) = x.parse::<usize>() {
                    return open_trip_transition(app, idx);
                } else if x == "close" {
                    return Transition::Pop;
                } else if self.table_tabs.handle_action(ctx, &x, &mut self.panel) {
                    // if true, tabs handled the action
                } else if x == "filter starts" {
                    // Set the dirty bit, so we re-apply the filters when the selector state is
                    // done.
                    self.dirty = true;
                    return Transition::Push(RectangularSelector::new(
                        ctx,
                        self.panel.stash("starts_in"),
                    ));
                } else if x == "filter ends" {
                    self.dirty = true;
                    return Transition::Push(RectangularSelector::new(
                        ctx,
                        self.panel.stash("ends_in"),
                    ));
                } else {
                    unreachable!("unhandled action: {}", x)
                }
            }
            Outcome::Changed => {
                if let Some(t) = self.tab.transition(ctx, app, &self.panel) {
                    return t;
                }

                self.dirty = true;
            }
            _ => {}
        }

        if self.dirty {
            self.dirty = false;
            match self.table_tabs.active_tab_idx() {
                0 => {
                    self.finished_trips_table.panel_changed(&self.panel);
                    self.finished_trips_table
                        .replace_render(ctx, app, &mut self.panel);
                }
                1 => {
                    self.cancelled_trips_table.panel_changed(&self.panel);
                    self.cancelled_trips_table
                        .replace_render(ctx, app, &mut self.panel);
                }
                2 => {
                    self.unfinished_trips_table.panel_changed(&self.panel);
                    self.unfinished_trips_table
                        .replace_render(ctx, app, &mut self.panel);
                }
                _ => unreachable!(),
            }
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.panel.draw(g);
        preview_trip(g, app, &self.panel);
    }
}

struct FinishedTrip {
    id: TripID,
    mode: TripMode,
    modified: bool,
    capped: bool,
    start: TripEndpoint,
    end: TripEndpoint,
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
    start: TripEndpoint,
    end: TripEndpoint,
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
    starts_in: Option<Polygon>,
    ends_in: Option<Polygon>,
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
                start: trip.start,
                end: trip.end,
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
            start: trip.start,
            end: trip.end,
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
            starts_in: None,
            ends_in: None,
            unmodified_trips: true,
            modified_trips: true,
            uncapped_trips: true,
            capped_trips: true,
        },
        to_controls: Box::new(move |ctx, app, state| {
            Widget::col(vec![
                checkbox_per_mode(ctx, app, &state.modes),
                Widget::row(vec![
                    Toggle::switch(ctx, "starting off-map", None, state.off_map_starts),
                    Toggle::switch(ctx, "ending off-map", None, state.off_map_ends),
                    ctx.style().btn_plain.text("filter starts").build_def(ctx),
                    ctx.style().btn_plain.text("filter ends").build_def(ctx),
                    Stash::new("starts_in", state.starts_in.clone()),
                    Stash::new("ends_in", state.ends_in.clone()),
                    if app.primary.has_modified_trips {
                        Toggle::switch(
                            ctx,
                            "trips unmodified by experiment",
                            None,
                            state.unmodified_trips,
                        )
                    } else {
                        Widget::nothing()
                    },
                    if app.primary.has_modified_trips {
                        Toggle::switch(
                            ctx,
                            "trips modified by experiment",
                            None,
                            state.modified_trips,
                        )
                    } else {
                        Widget::nothing()
                    },
                    if any_congestion_caps {
                        Toggle::switch(
                            ctx,
                            "trips not affected by congestion caps",
                            None,
                            state.uncapped_trips,
                        )
                    } else {
                        Widget::nothing()
                    },
                    if any_congestion_caps {
                        Toggle::switch(
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
                starts_in: panel.clone_stashed("starts_in"),
                ends_in: panel.clone_stashed("ends_in"),
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
        apply: Box::new(|state, x, app| {
            if !state.modes.contains(&x.mode) {
                return false;
            }
            if !state.off_map_starts && matches!(x.start, TripEndpoint::Border(_)) {
                return false;
            }
            if !state.off_map_ends && matches!(x.end, TripEndpoint::Border(_)) {
                return false;
            }
            if let Some(ref polygon) = state.starts_in {
                if x.start
                    .pos(x.mode, true, &app.primary.map)
                    .map(|pos| !polygon.contains_pt(pos.pt(&app.primary.map)))
                    .unwrap_or(true)
                {
                    return false;
                }
            }
            if let Some(ref polygon) = state.ends_in {
                if x.end
                    .pos(x.mode, false, &app.primary.map)
                    .map(|pos| !polygon.contains_pt(pos.pt(&app.primary.map)))
                    .unwrap_or(true)
                {
                    return false;
                }
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
        "finished_trips_table",
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
        Box::new(|ctx, _, x| Text::from(x.departure.ampm_tostring()).render(ctx)),
        Col::Sortable(Box::new(|rows| rows.sort_by_key(|x| x.departure))),
    );
    table.column(
        "Duration",
        Box::new(|ctx, app, x| Text::from(x.duration_after.to_string(&app.opts.units)).render(ctx)),
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
                Text::from(if x.duration_after == x.duration_before {
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
                })
                .render(ctx)
            }),
            Col::Sortable(Box::new(|rows| {
                rows.sort_by_key(|x| (100.0 * (x.duration_after / x.duration_before)) as isize)
            })),
        );
    }

    table.column(
        "Time spent waiting",
        Box::new(|ctx, app, x| Text::from(x.waiting.to_string(&app.opts.units)).render(ctx)),
        Col::Sortable(Box::new(|rows| rows.sort_by_key(|x| x.waiting))),
    );
    table.column(
        "Percent waiting",
        Box::new(|ctx, _, x| Text::from(x.percent_waiting.to_string()).render(ctx)),
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
            starts_in: None,
            ends_in: None,
            unmodified_trips: true,
            modified_trips: true,
            uncapped_trips: true,
            capped_trips: true,
        },
        to_controls: Box::new(move |ctx, app, state| {
            Widget::col(vec![
                checkbox_per_mode(ctx, app, &state.modes),
                Widget::row(vec![
                    Toggle::switch(ctx, "starting off-map", None, state.off_map_starts),
                    Toggle::switch(ctx, "ending off-map", None, state.off_map_ends),
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
                starts_in: panel.clone_stashed("starts_in"),
                ends_in: panel.clone_stashed("ends_in"),
                unmodified_trips: true,
                modified_trips: true,
                uncapped_trips: true,
                capped_trips: true,
            }
        }),
        apply: Box::new(|state, x, app| {
            if !state.modes.contains(&x.mode) {
                return false;
            }
            if !state.off_map_starts && matches!(x.start, TripEndpoint::Border(_)) {
                return false;
            }
            if !state.off_map_ends && matches!(x.end, TripEndpoint::Border(_)) {
                return false;
            }
            if let Some(ref polygon) = state.starts_in {
                if x.start
                    .pos(x.mode, true, &app.primary.map)
                    .map(|pos| !polygon.contains_pt(pos.pt(&app.primary.map)))
                    .unwrap_or(true)
                {
                    return false;
                }
            }
            if let Some(ref polygon) = state.ends_in {
                if x.end
                    .pos(x.mode, false, &app.primary.map)
                    .map(|pos| !polygon.contains_pt(pos.pt(&app.primary.map)))
                    .unwrap_or(true)
                {
                    return false;
                }
            }
            true
        }),
    };

    let mut table = Table::new(
        "cancelled_trips_table",
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
        Box::new(|ctx, _, x| Text::from(x.departure.ampm_tostring()).render(ctx)),
        Col::Sortable(Box::new(|rows| rows.sort_by_key(|x| x.departure))),
    );
    if app.has_prebaked().is_some() {
        table.column(
            "Estimated duration",
            Box::new(|ctx, app, x| {
                Text::from(x.duration_before.to_string(&app.opts.units)).render(ctx)
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
            starts_in: None,
            ends_in: None,
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
                starts_in: panel.clone_stashed("starts_in"),
                ends_in: panel.clone_stashed("ends_in"),
                unmodified_trips: true,
                modified_trips: true,
                uncapped_trips: true,
                capped_trips: true,
            }
        }),
        apply: Box::new(|state, x, _| {
            if !state.modes.contains(&x.mode) {
                return false;
            }
            true
        }),
    };

    let mut table = Table::new(
        "unfinished_trips_table",
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
        Box::new(|ctx, _, x| Text::from(x.departure.ampm_tostring()).render(ctx)),
        Col::Sortable(Box::new(|rows| rows.sort_by_key(|x| x.departure))),
    );
    if app.has_prebaked().is_some() {
        table.column(
            "Estimated duration",
            Box::new(|ctx, app, x| {
                Text::from(x.duration_before.to_string(&app.opts.units)).render(ctx)
            }),
            Col::Sortable(Box::new(|rows| rows.sort_by_key(|x| x.duration_before))),
        );
    }

    table
}
