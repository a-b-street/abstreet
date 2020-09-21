use crate::app::App;
use crate::game::{DrawBaselayer, State, Transition};
use crate::helpers::{
    checkbox_per_mode, cmp_duration_shorter, color_for_mode, color_for_trip_phase,
};
use crate::info::{OpenTrip, Tab};
use crate::sandbox::dashboards::table::{Col, Filter, Table};
use crate::sandbox::dashboards::DashTab;
use crate::sandbox::SandboxMode;
use abstutil::prettyprint_usize;
use geom::{Distance, Duration, Pt2D, Time};
use sim::{TripEndpoint, TripID, TripMode};
use std::collections::{BTreeSet, HashMap};
use widgetry::{
    Checkbox, Color, EventCtx, Filler, GeomBatch, GfxCtx, Line, Outcome, Panel, RewriteColor,
    ScreenPt, Text, Widget,
};

pub struct TripTable {
    table: Table<FinishedTrip, Filters>,
    panel: Panel,
}

impl TripTable {
    pub fn new(ctx: &mut EventCtx, app: &App) -> Box<dyn State> {
        let table = make_table_finished_trips(app);
        let panel = make_panel(ctx, app, &table);
        Box::new(TripTable { table, panel })
    }

    fn recalc(&mut self, ctx: &mut EventCtx, app: &App) {
        let mut new = make_panel(ctx, app, &self.table);
        new.restore(ctx, &self.panel);
        self.panel = new;
    }
}

impl State for TripTable {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => {
                if self.table.clicked(&x) {
                    self.recalc(ctx, app);
                } else if let Ok(idx) = x.parse::<usize>() {
                    let trip = TripID(idx);
                    let person = app.primary.sim.trip_to_person(trip);
                    return Transition::Multi(vec![
                        Transition::Pop,
                        Transition::ModifyState(Box::new(move |state, ctx, app| {
                            let sandbox = state.downcast_mut::<SandboxMode>().unwrap();
                            let mut actions = sandbox.contextual_actions();
                            sandbox.controls.common.as_mut().unwrap().launch_info_panel(
                                ctx,
                                app,
                                Tab::PersonTrips(person, OpenTrip::single(trip)),
                                &mut actions,
                            );
                        })),
                    ]);
                } else {
                    return DashTab::TripTable.transition(ctx, app, &x);
                }
            }
            Outcome::Changed => {
                self.table.panel_changed(&self.panel);
                self.recalc(ctx, app);
            }
            _ => {}
        }

        Transition::Keep
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::Custom
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        g.clear(app.cs.dialog_bg);
        self.panel.draw(g);
        preview_trip(g, app, &self.panel);
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
    _id: TripID,
    // TODO Original mode
    _departure: Time,
}

struct UnfinishedTrip {
    _id: TripID,
    _mode: TripMode,
    _departure: Time,
    _duration_before: Duration,
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

fn produce_raw_data(app: &App) -> (Vec<FinishedTrip>, Vec<CancelledTrip>, Vec<UnfinishedTrip>) {
    let mut finished = Vec::new();
    let mut cancelled = Vec::new();
    let unfinished = Vec::new();

    // Only make one pass through prebaked data
    let trip_times_before = if app.has_prebaked().is_some() {
        let mut times = HashMap::new();
        for (_, id, maybe_mode, dt) in &app.prebaked().finished_trips {
            if maybe_mode.is_some() {
                times.insert(*id, *dt);
            }
        }
        Some(times)
    } else {
        None
    };

    let sim = &app.primary.sim;
    for (_, id, maybe_mode, duration_after) in &sim.get_analytics().finished_trips {
        let trip = sim.trip_info(*id);

        let mode = if let Some(m) = maybe_mode {
            *m
        } else {
            cancelled.push(CancelledTrip {
                _id: *id,
                _departure: trip.departure,
            });
            continue;
        };

        let starts_off_map = match trip.start {
            TripEndpoint::Border(_, _) => true,
            _ => false,
        };
        let ends_off_map = match trip.end {
            TripEndpoint::Border(_, _) => true,
            _ => false,
        };

        let (_, waiting) = sim.finished_trip_time(*id).unwrap();
        let duration_before = if let Some(ref times) = trip_times_before {
            if let Some(dt) = times.get(id) {
                *dt
            } else {
                cancelled.push(CancelledTrip {
                    _id: *id,
                    _departure: trip.departure,
                });
                continue;
            }
        } else {
            Duration::ZERO
        };

        finished.push(FinishedTrip {
            id: *id,
            mode,
            departure: trip.departure,
            modified: trip.modified,
            capped: trip.capped,
            starts_off_map,
            ends_off_map,
            duration_after: *duration_after,
            duration_before,
            waiting,
            percent_waiting: (100.0 * waiting / *duration_after) as usize,
        });
    }

    (finished, cancelled, unfinished)
}

fn make_table_finished_trips(app: &App) -> Table<FinishedTrip, Filters> {
    let (finished, _, _) = produce_raw_data(app);
    let any_congestion_caps = app
        .primary
        .map
        .all_zones()
        .iter()
        .any(|z| z.restrictions.cap_vehicles_per_hour.is_some());
    let filter: Filter<FinishedTrip, Filters> = Filter {
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
            // TODO One big boolean expression?
            let mut ok = true;
            if !state.modes.contains(&x.mode) {
                ok = false;
            }
            if !state.off_map_starts && x.starts_off_map {
                ok = false;
            }
            if !state.off_map_ends && x.ends_off_map {
                ok = false;
            }
            if !state.unmodified_trips && !x.modified {
                ok = false;
            }
            if !state.modified_trips && x.modified {
                ok = false;
            }
            if !state.uncapped_trips && !x.capped {
                ok = false;
            }
            if !state.capped_trips && x.capped {
                ok = false;
            }
            ok
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
            Text::from(Line(x.mode.ongoing_verb()).fg(color_for_mode(app, x.mode))).render_ctx(ctx)
        }),
        Col::Static,
    );
    table.column(
        "Departure",
        Box::new(|ctx, _, x| Text::from(Line(x.departure.ampm_tostring())).render_ctx(ctx)),
        Col::Sortable(Box::new(|rows| rows.sort_by_key(|x| x.departure))),
    );
    table.column(
        "Duration",
        Box::new(|ctx, _, x| Text::from(Line(x.duration_after.to_string())).render_ctx(ctx)),
        Col::Sortable(Box::new(|rows| rows.sort_by_key(|x| x.duration_after))),
    );

    if app.has_prebaked().is_some() {
        table.column(
            "Comparison",
            Box::new(|ctx, _, x| {
                Text::from_all(cmp_duration_shorter(x.duration_after, x.duration_before))
                    .render_ctx(ctx)
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
                .render_ctx(ctx)
            }),
            Col::Sortable(Box::new(|rows| {
                rows.sort_by_key(|x| (100.0 * (x.duration_after / x.duration_before)) as isize)
            })),
        );
    }

    table.column(
        "Time spent waiting",
        Box::new(|ctx, _, x| Text::from(Line(x.waiting.to_string())).render_ctx(ctx)),
        Col::Sortable(Box::new(|rows| rows.sort_by_key(|x| x.waiting))),
    );
    table.column(
        "Percent waiting",
        Box::new(|ctx, _, x| Text::from(Line(x.percent_waiting.to_string())).render_ctx(ctx)),
        Col::Sortable(Box::new(|rows| rows.sort_by_key(|x| x.percent_waiting))),
    );

    table
}

fn make_panel(ctx: &mut EventCtx, app: &App, table: &Table<FinishedTrip, Filters>) -> Panel {
    let (finished, unfinished) = app.primary.sim.num_trips();
    let mut aborted = 0;
    // TODO Can we avoid iterating through this again?
    for (_, _, maybe_mode, _) in &app.primary.sim.get_analytics().finished_trips {
        if maybe_mode.is_none() {
            aborted += 1;
        }
    }

    let mut col = vec![DashTab::TripTable.picker(ctx, app)];

    col.push(Widget::custom_row(vec![
        Text::from(
            Line(format!(
                "{} ({:.1}%) Finished Trips",
                prettyprint_usize(finished),
                (finished as f64) / ((finished + aborted + unfinished) as f64) * 100.0
            ))
            .underlined(),
        )
        .draw(ctx)
        .margin_right(28),
        Text::from(Line(format!("{} Canceled Trips", prettyprint_usize(aborted))).secondary())
            .draw(ctx)
            .margin_right(28),
        Text::from(
            Line(format!(
                "{} ({:.1}%) Unfinished Trips",
                prettyprint_usize(unfinished),
                (unfinished as f64) / ((finished + aborted + unfinished) as f64) * 100.0
            ))
            .secondary(),
        )
        .draw(ctx),
    ]));

    col.push(table.render(ctx, app));

    col.push(
        Filler::square_width(ctx, 0.15)
            .named("preview")
            .centered_horiz(),
    );

    Panel::new(Widget::col(col))
        .exact_size_percent(90, 90)
        .build(ctx)
}

pub fn preview_trip(g: &mut GfxCtx, app: &App, panel: &Panel) {
    let inner_rect = panel.rect_of("preview").clone();
    let map_bounds = app.primary.map.get_bounds().clone();
    let zoom = 0.15 * g.canvas.window_width / map_bounds.width().max(map_bounds.height());
    g.fork(
        Pt2D::new(map_bounds.min_x, map_bounds.min_y),
        ScreenPt::new(inner_rect.x1, inner_rect.y1),
        zoom,
        None,
    );
    g.enable_clipping(inner_rect);

    g.redraw(&app.primary.draw_map.boundary_polygon);
    g.redraw(&app.primary.draw_map.draw_all_areas);
    g.redraw(
        &app.primary
            .draw_map
            .draw_all_unzoomed_roads_and_intersections,
    );

    if let Some(x) = panel.currently_hovering() {
        if let Ok(idx) = x.parse::<usize>() {
            let trip = TripID(idx);
            preview_route(g, app, trip).draw(g);
        }
    }

    g.disable_clipping();
    g.unfork();
}

fn preview_route(g: &mut GfxCtx, app: &App, id: TripID) -> GeomBatch {
    let mut batch = GeomBatch::new();
    for p in app
        .primary
        .sim
        .get_analytics()
        .get_trip_phases(id, &app.primary.map)
    {
        if let Some((dist, ref path)) = p.path {
            if let Some(trace) = path.trace(&app.primary.map, dist, None) {
                batch.push(
                    color_for_trip_phase(app, p.phase_type),
                    trace.make_polygons(Distance::meters(20.0)),
                );
            }
        }
    }

    let trip = app.primary.sim.trip_info(id);
    batch.append(
        GeomBatch::load_svg(g.prerender, "system/assets/timeline/start_pos.svg")
            .scale(10.0)
            .color(RewriteColor::Change(Color::WHITE, Color::BLACK))
            .color(RewriteColor::Change(
                Color::hex("#5B5B5B"),
                Color::hex("#CC4121"),
            ))
            .centered_on(match trip.start {
                TripEndpoint::Bldg(b) => app.primary.map.get_b(b).label_center,
                TripEndpoint::Border(i, _) => app.primary.map.get_i(i).polygon.center(),
            }),
    );
    batch.append(
        GeomBatch::load_svg(g.prerender, "system/assets/timeline/goal_pos.svg")
            .scale(10.0)
            .color(RewriteColor::Change(Color::WHITE, Color::BLACK))
            .color(RewriteColor::Change(
                Color::hex("#5B5B5B"),
                Color::hex("#CC4121"),
            ))
            .centered_on(match trip.end {
                TripEndpoint::Bldg(b) => app.primary.map.get_b(b).label_center,
                TripEndpoint::Border(i, _) => app.primary.map.get_i(i).polygon.center(),
            }),
    );

    batch
}
