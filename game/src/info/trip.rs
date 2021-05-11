use std::collections::BTreeMap;

use maplit::btreemap;

use geom::{Distance, Duration, Percent, Polygon, Pt2D};
use map_gui::ID;
use map_model::{Map, Path, PathStep, Traversable};
use sim::{
    AgentID, Analytics, PersonID, Problem, TripEndpoint, TripID, TripInfo, TripMode, TripPhase,
    TripPhaseType,
};
use widgetry::{
    Color, ControlState, DrawWithTooltips, EventCtx, GeomBatch, Line, LinePlot, PlotOptions,
    RewriteColor, Series, Text, TextExt, Widget,
};

use crate::app::App;
use crate::common::color_for_trip_phase;
use crate::info::{make_table, Details, Tab};

#[derive(Clone)]
pub struct OpenTrip {
    pub show_after: bool,
    // (unzoomed, zoomed). Indexed by order of TripPhase.
    cached_routes: Vec<Option<(Polygon, Vec<Polygon>)>>,
}

// Ignore cached_routes
impl std::cmp::PartialEq for OpenTrip {
    fn eq(&self, other: &OpenTrip) -> bool {
        self.show_after == other.show_after
    }
}

impl OpenTrip {
    pub fn single(id: TripID) -> BTreeMap<TripID, OpenTrip> {
        btreemap! { id => OpenTrip::new() }
    }

    pub fn new() -> OpenTrip {
        OpenTrip {
            show_after: true,
            cached_routes: Vec::new(),
        }
    }
}

pub fn ongoing(
    ctx: &mut EventCtx,
    app: &App,
    id: TripID,
    agent: AgentID,
    open_trip: &mut OpenTrip,
    details: &mut Details,
) -> Widget {
    let phases = app
        .primary
        .sim
        .get_analytics()
        .get_trip_phases(id, &app.primary.map);
    let trip = app.primary.sim.trip_info(id);

    let col_width = Percent::int(7);
    let props = app.primary.sim.agent_properties(&app.primary.map, agent);
    let activity = agent.to_type().ongoing_verb();
    let time_so_far = app.primary.sim.time() - trip.departure;

    let mut col = Vec::new();

    {
        col.push(Widget::custom_row(vec![
            Widget::custom_row(vec![Line("Trip time").secondary().into_widget(ctx)])
                .force_width_pct(ctx, col_width),
            Text::from_all(vec![
                Line(props.total_time.to_string(&app.opts.units)),
                Line(format!(
                    " {} / {} this trip",
                    activity,
                    time_so_far.to_string(&app.opts.units)
                ))
                .secondary(),
            ])
            .into_widget(ctx),
        ]));
    }
    {
        col.push(Widget::custom_row(vec![
            Widget::custom_row(vec![Line("Distance").secondary().into_widget(ctx)])
                .force_width_pct(ctx, col_width),
            Text::from_all(vec![
                Line(props.dist_crossed.to_string(&app.opts.units)),
                Line(format!("/{}", props.total_dist.to_string(&app.opts.units))).secondary(),
            ])
            .into_widget(ctx),
        ]));
    }
    {
        col.push(Widget::custom_row(vec![
            Line("Waiting")
                .secondary()
                .into_widget(ctx)
                .container()
                .force_width_pct(ctx, col_width),
            Widget::col(vec![
                format!("{} here", props.waiting_here.to_string(&app.opts.units)).text_widget(ctx),
                Text::from_all(vec![
                    if props.total_waiting != Duration::ZERO {
                        Line(format!(
                            "{}%",
                            (100.0 * (props.total_waiting / time_so_far)) as usize
                        ))
                    } else {
                        Line("0%")
                    },
                    Line(format!(" total of {} time spent waiting", activity)).secondary(),
                ])
                .into_widget(ctx),
            ]),
        ]));
    }
    col.push(describe_problems(
        ctx,
        app.primary.sim.get_analytics(),
        id,
        &trip,
        col_width,
    ));
    {
        col.push(Widget::custom_row(vec![
            Widget::custom_row(vec![Line("Purpose").secondary().into_widget(ctx)])
                .force_width_pct(ctx, col_width),
            Line(trip.purpose.to_string()).secondary().into_widget(ctx),
        ]));
    }

    col.push(make_trip_details(
        ctx,
        app,
        id,
        open_trip,
        details,
        phases,
        &app.primary.map,
        Some(props.dist_crossed / props.total_dist),
    ));
    Widget::col(col)
}

pub fn future(
    ctx: &mut EventCtx,
    app: &App,
    id: TripID,
    open_trip: &mut OpenTrip,
    details: &mut Details,
) -> Widget {
    let trip = app.primary.sim.trip_info(id);

    let mut col = Vec::new();

    let now = app.primary.sim.time();
    if now > trip.departure {
        col.extend(make_table(
            ctx,
            vec![(
                "Start delayed",
                (now - trip.departure).to_string(&app.opts.units),
            )],
        ));
    }

    if let Some(estimated_trip_time) = app
        .has_prebaked()
        .and_then(|_| app.prebaked().finished_trip_time(id))
    {
        col.extend(make_table(
            ctx,
            vec![
                (
                    "Estimated trip time",
                    estimated_trip_time.to_string(&app.opts.units),
                ),
                ("Purpose", trip.purpose.to_string()),
            ],
        ));

        if let Some(t) = app.primary.calculate_unedited_map(ctx) {
            details.stop_immediately = Some(t);
            return Widget::nothing();
        }
        let borrow = app.primary.unedited_map.borrow();
        let unedited_map = borrow.as_ref().unwrap_or(&app.primary.map);
        let phases = app.prebaked().get_trip_phases(id, unedited_map);
        col.push(make_trip_details(
            ctx,
            app,
            id,
            open_trip,
            details,
            phases,
            unedited_map,
            None,
        ));
    } else {
        col.extend(make_table(ctx, vec![("Purpose", trip.purpose.to_string())]));

        col.push(make_trip_details(
            ctx,
            app,
            id,
            open_trip,
            details,
            Vec::new(),
            &app.primary.map,
            None,
        ));
    }

    Widget::col(col)
}

pub fn finished(
    ctx: &mut EventCtx,
    app: &App,
    person: PersonID,
    open_trips: &mut BTreeMap<TripID, OpenTrip>,
    id: TripID,
    details: &mut Details,
) -> Widget {
    // Weird order to make sure the borrow remains in scope in case we need it.
    if !open_trips[&id].show_after {
        if let Some(t) = app.primary.calculate_unedited_map(ctx) {
            details.stop_immediately = Some(t);
            return Widget::nothing();
        }
    }
    let borrow = app.primary.unedited_map.borrow();

    let trip = app.primary.sim.trip_info(id);
    let (phases, map_for_pathfinding) = if open_trips[&id].show_after {
        (
            app.primary
                .sim
                .get_analytics()
                .get_trip_phases(id, &app.primary.map),
            &app.primary.map,
        )
    } else {
        let unedited_map = borrow.as_ref().unwrap_or(&app.primary.map);
        (
            app.prebaked().get_trip_phases(id, unedited_map),
            unedited_map,
        )
    };

    let mut col = Vec::new();

    if open_trips[&id].show_after && app.has_prebaked().is_some() {
        let mut open = open_trips.clone();
        open.insert(
            id,
            OpenTrip {
                show_after: false,
                cached_routes: Vec::new(),
            },
        );
        details.hyperlinks.insert(
            format!("show before changes for {}", id),
            Tab::PersonTrips(person, open),
        );
        col.push(
            ctx.style()
                .btn_outline
                .btn()
                .label_styled_text(
                    Text::from_all(vec![
                        Line("After / "),
                        Line("Before").secondary(),
                        Line(" "),
                        Line(&app.primary.map.get_edits().edits_name).underlined(),
                    ]),
                    ControlState::Default,
                )
                .build_widget(ctx, format!("show before changes for {}", id)),
        );
    } else if app.has_prebaked().is_some() {
        let mut open = open_trips.clone();
        open.insert(id, OpenTrip::new());
        details.hyperlinks.insert(
            format!("show after changes for {}", id),
            Tab::PersonTrips(person, open),
        );
        col.push(
            ctx.style()
                .btn_outline
                .btn()
                .label_styled_text(
                    Text::from_all(vec![
                        Line("After / ").secondary(),
                        Line("Before"),
                        Line(" "),
                        Line(&app.primary.map.get_edits().edits_name).underlined(),
                    ]),
                    ControlState::Default,
                )
                .build_widget(ctx, format!("show after changes for {}", id)),
        );
    }

    let col_width = Percent::int(15);
    {
        if let Some(end_time) = phases.last().as_ref().and_then(|p| p.end_time) {
            col.push(Widget::custom_row(vec![
                Widget::custom_row(vec![Line("Trip time").secondary().into_widget(ctx)])
                    .force_width_pct(ctx, col_width),
                (end_time - trip.departure)
                    .to_string(&app.opts.units)
                    .text_widget(ctx),
            ]));
        } else {
            col.push(Widget::custom_row(vec![
                Widget::custom_row(vec![Line("Trip time").secondary().into_widget(ctx)])
                    .force_width_pct(ctx, col_width),
                "Trip didn't complete before map changes".text_widget(ctx),
            ]));
        }

        // TODO This is always the waiting time in the current simulation, even if we've chosen to
        // look at the prebaked results! Misleading -- change the text.
        let (_, waiting, _) = app.primary.sim.finished_trip_details(id).unwrap();
        col.push(Widget::custom_row(vec![
            Widget::custom_row(vec![Line("Total waiting time")
                .secondary()
                .into_widget(ctx)])
            .force_width_pct(ctx, col_width),
            waiting.to_string(&app.opts.units).text_widget(ctx),
        ]));

        col.push(Widget::custom_row(vec![
            Widget::custom_row(vec![Line("Purpose").secondary().into_widget(ctx)])
                .force_width_pct(ctx, col_width),
            Line(trip.purpose.to_string()).secondary().into_widget(ctx),
        ]));
    }
    col.push(describe_problems(
        ctx,
        if open_trips[&id].show_after {
            app.primary.sim.get_analytics()
        } else {
            app.prebaked()
        },
        id,
        &trip,
        col_width,
    ));

    col.push(make_trip_details(
        ctx,
        app,
        id,
        open_trips.get_mut(&id).unwrap(),
        details,
        phases,
        map_for_pathfinding,
        None,
    ));
    Widget::col(col)
}

pub fn cancelled(
    ctx: &mut EventCtx,
    app: &App,
    id: TripID,
    open_trip: &mut OpenTrip,
    details: &mut Details,
) -> Widget {
    let trip = app.primary.sim.trip_info(id);

    let mut col = vec![Text::from(format!(
        "Trip cancelled: {}",
        trip.cancellation_reason.as_ref().unwrap()
    ))
    .wrap_to_pct(ctx, 20)
    .into_widget(ctx)];

    col.extend(make_table(ctx, vec![("Purpose", trip.purpose.to_string())]));

    col.push(make_trip_details(
        ctx,
        app,
        id,
        open_trip,
        details,
        Vec::new(),
        &app.primary.map,
        None,
    ));

    Widget::col(col)
}

fn describe_problems(
    ctx: &mut EventCtx,
    analytics: &Analytics,
    id: TripID,
    trip: &TripInfo,
    col_width: Percent,
) -> Widget {
    if trip.mode == TripMode::Bike {
        let mut count_large_intersections = 0;
        let mut count_overtakes = 0;
        let empty = Vec::new();
        for (_, problem) in analytics.problems_per_trip.get(&id).unwrap_or(&empty) {
            match problem {
                Problem::LargeIntersectionCrossing(_) => {
                    count_large_intersections += 1;
                }
                Problem::OvertakeDesired(_) => {
                    count_overtakes += 1;
                }
                Problem::IntersectionDelay(_, _) => {}
            }
        }
        let mut txt = Text::new();
        txt.add_appended(vec![
            Line(count_large_intersections.to_string()),
            if count_large_intersections == 1 {
                Line(" crossing at large intersections")
            } else {
                Line(" crossings at large intersections")
            }
            .secondary(),
        ]);
        txt.add_appended(vec![
            Line(count_overtakes.to_string()),
            if count_overtakes == 1 {
                Line(" vehicle wanted to over-take")
            } else {
                Line(" vehicles wanted to over-take")
            }
            .secondary(),
        ]);

        Widget::custom_row(vec![
            Line("Risk Exposure")
                .secondary()
                .into_widget(ctx)
                .container()
                .force_width_pct(ctx, col_width),
            txt.into_widget(ctx),
        ])
    } else {
        Widget::nothing()
    }
}

fn draw_problems(ctx: &EventCtx, app: &App, details: &mut Details, id: TripID) {
    let map = &app.primary.map;
    let empty = Vec::new();
    for (_, problem) in app
        .primary
        .sim
        .get_analytics()
        .problems_per_trip
        .get(&id)
        .unwrap_or(&empty)
    {
        match problem {
            Problem::IntersectionDelay(i, delay) => {
                let i = map.get_i(*i);
                // TODO These thresholds don't match what we use as thresholds in the simulation.
                let (slow, slower) = if i.is_traffic_signal() {
                    (Duration::seconds(30.0), Duration::minutes(2))
                } else {
                    (Duration::seconds(5.0), Duration::seconds(30.0))
                };
                let (fg_color, bg_color) = if *delay < slow {
                    (Color::WHITE, app.cs.slow_intersection)
                } else if *delay < slower {
                    (Color::BLACK, app.cs.slower_intersection)
                } else {
                    (Color::WHITE, app.cs.slowest_intersection)
                };
                details.unzoomed.append(
                    Text::from(Line(format!("{}", delay)).fg(fg_color))
                        .bg(bg_color)
                        .render(ctx)
                        .centered_on(i.polygon.center()),
                );
                details.zoomed.append(
                    Text::from(Line(format!("{}", delay)).fg(fg_color))
                        .bg(bg_color)
                        .render(ctx)
                        .scale(0.4)
                        .centered_on(i.polygon.center()),
                );
                details.tooltips.push((
                    i.polygon.clone(),
                    Text::from(Line(format!("{} delay here", delay))),
                ));
            }
            Problem::LargeIntersectionCrossing(i) => {
                let i = map.get_i(*i);
                details.unzoomed.append(
                    GeomBatch::load_svg(ctx, "system/assets/tools/alert.svg")
                        .centered_on(i.polygon.center())
                        .color(RewriteColor::ChangeAlpha(0.8)),
                );
                details.zoomed.append(
                    GeomBatch::load_svg(ctx, "system/assets/tools/alert.svg")
                        .scale(0.5)
                        .color(RewriteColor::ChangeAlpha(0.5))
                        .centered_on(i.polygon.center()),
                );
                details.tooltips.push((
                    i.polygon.clone(),
                    Text::from_multiline(vec![
                        Line(format!("This intersection has {} legs", i.roads.len())),
                        Line("This has an increased risk of crash or injury for cyclists"),
                        Line("Source: 2020 Seattle DOT Safety Analysis"),
                    ]),
                ));
            }
            Problem::OvertakeDesired(on) => {
                let pt = on.get_polyline(map).middle();
                details.unzoomed.append(
                    GeomBatch::load_svg(ctx, "system/assets/tools/alert.svg")
                        .centered_on(pt)
                        .color(RewriteColor::ChangeAlpha(0.8)),
                );
                details.zoomed.append(
                    GeomBatch::load_svg(ctx, "system/assets/tools/alert.svg")
                        .scale(0.5)
                        .color(RewriteColor::ChangeAlpha(0.5))
                        .centered_on(pt),
                );
                details.tooltips.push((
                    match on {
                        Traversable::Lane(l) => map.get_parent(*l).get_thick_polygon(map),
                        Traversable::Turn(t) => map.get_i(t.parent).polygon.clone(),
                    },
                    Text::from("A vehicle wanted to over-take this cyclist near here."),
                ));
            }
        }
    }
}

/// Draws the timeline for a single trip, with tooltips
fn make_timeline(
    ctx: &mut EventCtx,
    app: &App,
    trip_id: TripID,
    phases: &Vec<TripPhase>,
    progress_along_path: Option<f64>,
) -> Widget {
    let map = &app.primary.map;
    let sim = &app.primary.sim;

    let total_width = 0.22 * ctx.canvas.window_width;
    let trip = sim.trip_info(trip_id);
    let end_time = phases.last().as_ref().and_then(|p| p.end_time);
    let total_duration_so_far = end_time.unwrap_or_else(|| sim.time()) - trip.departure;

    // Represent each phase of a trip as a rectangular segment, with width proportional to the
    // phase's duration. As we go through, build up one batch to draw containing the rectangles and
    // icons above them.
    let mut batch = GeomBatch::new();
    // And associate a tooltip with each rectangle segment
    let mut tooltips: Vec<(Polygon, Text)> = Vec::new();
    // How far along are we from previous segments?
    let mut x1 = 0.0;
    let rectangle_height = 15.0;
    let icon_height = 30.0;
    for (idx, p) in phases.into_iter().enumerate() {
        let mut tooltip = vec![
            p.phase_type.describe(map),
            format!("  Started at {}", p.start_time.ampm_tostring()),
        ];
        let phase_duration = if let Some(t2) = p.end_time {
            let d = t2 - p.start_time;
            tooltip.push(format!(
                "  Ended at {} (duration: {})",
                t2.ampm_tostring(),
                d
            ));
            d
        } else {
            let d = sim.time() - p.start_time;
            tooltip.push(format!(
                "  Ongoing (duration so far: {})",
                d.to_string(&app.opts.units)
            ));
            d
        };
        // TODO Problems when this is really low?
        let percent_duration = if total_duration_so_far == Duration::ZERO {
            0.0
        } else {
            phase_duration / total_duration_so_far
        };
        tooltip.push(format!(
            "  {}% of trip percentage",
            (100.0 * percent_duration) as usize
        ));
        tooltip.push(format!(
            "  Total delayed time {}",
            sim.trip_blocked_time(trip_id)
        ));

        let phase_width = total_width * percent_duration;
        let rectangle =
            Polygon::rectangle(phase_width, rectangle_height).translate(x1, icon_height);

        tooltips.push((
            rectangle.clone(),
            Text::from_multiline(tooltip.into_iter().map(Line).collect()),
        ));

        batch.push(
            color_for_trip_phase(app, p.phase_type).alpha(0.7),
            rectangle,
        );
        if idx == phases.len() - 1 {
            // Show where we are in the trip currently
            if let Some(p) = progress_along_path {
                batch.append(
                    GeomBatch::load_svg(ctx, "system/assets/timeline/current_pos.svg").centered_on(
                        Pt2D::new(x1 + p * phase_width, icon_height + (rectangle_height / 2.0)),
                    ),
                );
            }
        }
        batch.append(
            GeomBatch::load_svg(
                ctx.prerender,
                match p.phase_type {
                    TripPhaseType::Driving => "system/assets/timeline/driving.svg",
                    TripPhaseType::Walking => "system/assets/timeline/walking.svg",
                    TripPhaseType::Biking => "system/assets/timeline/biking.svg",
                    TripPhaseType::Parking => "system/assets/timeline/parking.svg",
                    TripPhaseType::WaitingForBus(_, _) => {
                        "system/assets/timeline/waiting_for_bus.svg"
                    }
                    TripPhaseType::RidingBus(_, _, _) => "system/assets/timeline/riding_bus.svg",
                    TripPhaseType::Cancelled | TripPhaseType::Finished => unreachable!(),
                    TripPhaseType::DelayedStart => "system/assets/timeline/delayed_start.svg",
                },
            )
            .centered_on(Pt2D::new(x1 + phase_width / 2.0, icon_height / 2.0)),
        );

        x1 += phase_width;
    }

    DrawWithTooltips::new(ctx, batch, tooltips, Box::new(|_| GeomBatch::new()))
}

/// Creates the timeline, location warp, and time warp buttons for one trip, and draws the route on
/// the map.
fn make_trip_details(
    ctx: &mut EventCtx,
    app: &App,
    trip_id: TripID,
    open_trip: &mut OpenTrip,
    details: &mut Details,
    phases: Vec<TripPhase>,
    map_for_pathfinding: &Map,
    progress_along_path: Option<f64>,
) -> Widget {
    let map = &app.primary.map;
    let sim = &app.primary.sim;
    let trip = sim.trip_info(trip_id);
    let end_time = phases.last().as_ref().and_then(|p| p.end_time);

    let start_btn = {
        let (id, center, name) = endpoint(&trip.start, app);
        details
            .warpers
            .insert(format!("jump to start of {}", trip_id), id);

        details
            .unzoomed
            .append(map_gui::tools::start_marker(ctx, center, 2.0));
        details
            .zoomed
            .append(map_gui::tools::start_marker(ctx, center, 0.5));

        ctx.style()
            .btn_plain
            .icon("system/assets/timeline/start_pos.svg")
            .image_color(RewriteColor::NoOp, ControlState::Default)
            .tooltip(name)
            .build_widget(ctx, format!("jump to start of {}", trip_id))
    };

    let goal_btn = {
        let (id, center, name) = endpoint(&trip.end, app);
        details
            .warpers
            .insert(format!("jump to goal of {}", trip_id), id);

        details
            .unzoomed
            .append(map_gui::tools::goal_marker(ctx, center, 2.0));
        details
            .zoomed
            .append(map_gui::tools::goal_marker(ctx, center, 0.5));

        ctx.style()
            .btn_plain
            .icon("system/assets/timeline/goal_pos.svg")
            .image_color(RewriteColor::NoOp, ControlState::Default)
            .tooltip(name)
            .build_widget(ctx, format!("jump to goal of {}", trip_id))
    };

    let timeline = make_timeline(ctx, app, trip_id, &phases, progress_along_path);
    let mut elevation = Vec::new();
    let mut path_impossible = false;
    for (idx, p) in phases.into_iter().enumerate() {
        let color = color_for_trip_phase(app, p.phase_type).alpha(0.7);
        if let Some(path) = &p.path {
            // Don't show the elevation plot for somebody walking to their car
            if ((trip.mode == TripMode::Walk || trip.mode == TripMode::Transit)
                && p.phase_type == TripPhaseType::Walking)
                || (trip.mode == TripMode::Bike && p.phase_type == TripPhaseType::Biking)
            {
                elevation.push(make_elevation(
                    ctx,
                    color,
                    p.phase_type == TripPhaseType::Walking,
                    path,
                    map,
                ));
            }

            // This is expensive, so cache please
            if idx == open_trip.cached_routes.len() {
                if let Some(trace) = path.trace(map_for_pathfinding) {
                    open_trip.cached_routes.push(Some((
                        trace.make_polygons(Distance::meters(10.0)),
                        trace.dashed_lines(
                            Distance::meters(0.75),
                            Distance::meters(1.0),
                            Distance::meters(0.4),
                        ),
                    )));
                } else {
                    open_trip.cached_routes.push(None);
                }
            }
            if let Some((ref unzoomed, ref zoomed)) = open_trip.cached_routes[idx] {
                details.unzoomed.push(color, unzoomed.clone());
                details.zoomed.extend(color, zoomed.clone());
            }
        } else if p.has_path_req {
            path_impossible = true;
        }
        // Just fill this in so the indexing doesn't mess up
        if idx == open_trip.cached_routes.len() {
            open_trip.cached_routes.push(None);
        }
    }

    let mut col = vec![
        // TODO Button alignment is off. The timeline has some negative coordinates...
        Widget::custom_row(vec![
            start_btn.align_bottom(),
            timeline,
            goal_btn.align_bottom(),
        ])
        .evenly_spaced(),
        Widget::row(vec![
            trip.departure.ampm_tostring().text_widget(ctx),
            if let Some(t) = end_time {
                t.ampm_tostring().text_widget(ctx).align_right()
            } else {
                Widget::nothing()
            },
        ]),
        Widget::row(vec![
            if details.can_jump_to_time {
                details.time_warpers.insert(
                    format!("jump to {}", trip.departure),
                    (trip_id, trip.departure),
                );
                ctx.style()
                    .btn_plain
                    .icon("system/assets/speed/jump_to_time.svg")
                    .tooltip({
                        let mut txt = Text::from("This will jump to ");
                        txt.append(Line(trip.departure.ampm_tostring()).fg(Color::hex("#F9EC51")));
                        txt.add_line("The simulation will continue, and your score");
                        txt.add_line("will be calculated at this new time.");
                        txt
                    })
                    .build_widget(ctx, format!("jump to {}", trip.departure))
            } else {
                Widget::nothing()
            },
            if let Some(t) = end_time {
                if details.can_jump_to_time {
                    details
                        .time_warpers
                        .insert(format!("jump to {}", t), (trip_id, t));
                    ctx.style()
                        .btn_plain
                        .icon("system/assets/speed/jump_to_time.svg")
                        .tooltip({
                            let mut txt = Text::from("This will jump to ");
                            txt.append(Line(t.ampm_tostring()).fg(Color::hex("#F9EC51")));
                            txt.add_line("The simulation will continue, and your score");
                            txt.add_line("will be calculated at this new time.");
                            txt
                        })
                        .build_widget(ctx, format!("jump to {}", t))
                        .align_right()
                } else {
                    Widget::nothing()
                }
            } else {
                Widget::nothing()
            },
        ]),
    ];
    if path_impossible {
        col.push("Map edits have disconnected the path taken before".text_widget(ctx));
    }
    col.extend(elevation);
    draw_problems(ctx, app, details, trip_id);
    Widget::col(col)
}

fn make_elevation(ctx: &EventCtx, color: Color, walking: bool, path: &Path, map: &Map) -> Widget {
    let mut pts: Vec<(Distance, Distance)> = Vec::new();
    let mut dist = Distance::ZERO;
    for step in path.get_steps() {
        if let PathStep::Turn(t) = step {
            pts.push((dist, map.get_i(t.parent).elevation));
        }
        dist += step.as_traversable().get_polyline(map).length();
    }
    // TODO Show roughly where we are in the trip; use distance covered by current path for this
    LinePlot::new(
        ctx,
        vec![Series {
            label: if walking {
                "Elevation for walking"
            } else {
                "Elevation for biking"
            }
            .to_string(),
            color,
            pts,
        }],
        PlotOptions::fixed(),
    )
}

// (ID, center, name)
fn endpoint(endpt: &TripEndpoint, app: &App) -> (ID, Pt2D, String) {
    match endpt {
        TripEndpoint::Bldg(b) => {
            let bldg = app.primary.map.get_b(*b);
            (ID::Building(*b), bldg.label_center, bldg.address.clone())
        }
        TripEndpoint::Border(i) => {
            let i = app.primary.map.get_i(*i);
            (
                ID::Intersection(i.id),
                i.polygon.center(),
                format!(
                    "off map, via {}",
                    i.name(app.opts.language.as_ref(), &app.primary.map)
                ),
            )
        }
        TripEndpoint::SuddenlyAppear(pos) => (
            ID::Lane(pos.lane()),
            pos.pt(&app.primary.map),
            format!(
                "suddenly appear {} along",
                pos.dist_along().to_string(&app.opts.units)
            ),
        ),
    }
}
