use std::collections::BTreeMap;

use maplit::btreemap;

use geom::{ArrowCap, Distance, Duration, Percent, PolyLine, Polygon, Pt2D, Time};
use map_model::{Map, Path, PathStep};
use sim::{AgentID, PersonID, TripEndpoint, TripID, TripPhase, TripPhaseType};
use widgetry::{
    Btn, Color, DrawWithTooltips, EventCtx, GeomBatch, Line, LinePlot, PlotOptions, RewriteColor,
    Series, Text, TextExt, TextSpan, Widget,
};

use crate::app::App;
use crate::helpers::{color_for_trip_phase, ID};
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
    let props = app.primary.sim.agent_properties(agent);
    let activity = agent.to_type().ongoing_verb();
    let time_so_far = app.primary.sim.time() - trip.departure;

    let mut col = Vec::new();

    {
        col.push(Widget::custom_row(vec![
            Widget::custom_row(vec![Line("Trip time").secondary().draw(ctx)])
                .force_width_pct(ctx, col_width),
            Text::from_all(vec![
                Line(props.total_time.to_string()),
                Line(format!(" {} / {} this trip", activity, time_so_far)).secondary(),
            ])
            .draw(ctx),
        ]));
    }
    {
        col.push(Widget::custom_row(vec![
            Widget::custom_row(vec![Line("Distance").secondary().draw(ctx)])
                .force_width_pct(ctx, col_width),
            Widget::col(vec![
                Text::from_all(vec![
                    Line(props.dist_crossed.describe_rounded()),
                    Line(format!("/{}", props.total_dist.describe_rounded())).secondary(),
                ])
                .draw(ctx),
                Text::from_all(vec![
                    Line(format!("{} lanes", props.lanes_crossed)),
                    Line(format!("/{}", props.total_lanes)).secondary(),
                ])
                .draw(ctx),
            ]),
        ]));
    }
    {
        col.push(Widget::custom_row(vec![
            Line("Waiting")
                .secondary()
                .draw(ctx)
                .container()
                .force_width_pct(ctx, col_width),
            Widget::col(vec![
                format!("{} here", props.waiting_here).draw_text(ctx),
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
                .draw(ctx),
            ]),
        ]));
    }
    {
        col.push(Widget::custom_row(vec![
            Widget::custom_row(vec![Line("Purpose").secondary().draw(ctx)])
                .force_width_pct(ctx, col_width),
            Line(trip.purpose.to_string()).secondary().draw(ctx),
        ]));
    }

    col.push(make_timeline(
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
            vec![("Start delayed", (now - trip.departure).to_string())],
        ));
    }

    if let Some(estimated_trip_time) = app
        .has_prebaked()
        .and_then(|_| app.prebaked().finished_trip_time(id))
    {
        col.extend(make_table(
            ctx,
            vec![
                ("Estimated trip time", estimated_trip_time.to_string()),
                ("Purpose", trip.purpose.to_string()),
            ],
        ));

        app.primary.calculate_unedited_map();
        let borrow = app.primary.unedited_map.borrow();
        let unedited_map = borrow.as_ref().unwrap_or(&app.primary.map);
        let phases = app.prebaked().get_trip_phases(id, unedited_map);
        col.push(make_timeline(
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
        // TODO Warp buttons. make_table is showing its age.
        let (id1, _, name1) = endpoint(&trip.start, app);
        let (id2, _, name2) = endpoint(&trip.end, app);
        details
            .warpers
            .insert(format!("jump to start of {}", id), id1);
        details
            .warpers
            .insert(format!("jump to goal of {}", id), id2);
        if details.can_jump_to_time {
            details
                .time_warpers
                .insert(format!("wait for {}", id), (id, trip.departure));
        }

        col.push(
            Widget::row(vec![
                Btn::svg(
                    "system/assets/timeline/start_pos.svg",
                    RewriteColor::Change(Color::WHITE, app.cs.hovering),
                )
                .tooltip(Text::from(Line(name1)))
                .build(ctx, format!("jump to start of {}", id), None),
                if details.can_jump_to_time {
                    Btn::text_bg2("Wait for trip")
                        .tooltip(Text::from(Line(format!(
                            "This will advance the simulation to {}",
                            trip.departure.ampm_tostring()
                        ))))
                        .build(ctx, format!("wait for {}", id), None)
                } else {
                    Widget::nothing()
                },
                Btn::svg(
                    "system/assets/timeline/goal_pos.svg",
                    RewriteColor::Change(Color::WHITE, app.cs.hovering),
                )
                .tooltip(Text::from(Line(name2)))
                .build(ctx, format!("jump to goal of {}", id), None),
            ])
            .evenly_spaced(),
        );

        col.extend(make_table(
            ctx,
            vec![
                ("Departure", trip.departure.ampm_tostring()),
                ("Purpose", trip.purpose.to_string()),
            ],
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
        app.primary.calculate_unedited_map();
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
            Btn::text_bg(
                format!("show before changes for {}", id),
                Text::from_all(vec![
                    Line("After / "),
                    Line("Before").secondary(),
                    Line(" "),
                    Line(&app.primary.map.get_edits().edits_name).underlined(),
                ]),
                app.cs.section_bg,
                app.cs.hovering,
            )
            .build_def(ctx, None),
        );
    } else if app.has_prebaked().is_some() {
        let mut open = open_trips.clone();
        open.insert(id, OpenTrip::new());
        details.hyperlinks.insert(
            format!("show after changes for {}", id),
            Tab::PersonTrips(person, open),
        );
        col.push(
            Btn::text_bg(
                format!("show after changes for {}", id),
                Text::from_all(vec![
                    Line("After / ").secondary(),
                    Line("Before"),
                    Line(" "),
                    Line(&app.primary.map.get_edits().edits_name).underlined(),
                ]),
                app.cs.section_bg,
                app.cs.hovering,
            )
            .build_def(ctx, None),
        );
    }

    {
        let col_width = Percent::int(15);

        let total_trip_time =
            phases.last().as_ref().and_then(|p| p.end_time).unwrap() - trip.departure;
        col.push(Widget::custom_row(vec![
            Widget::custom_row(vec![Line("Trip time").secondary().draw(ctx)])
                .force_width_pct(ctx, col_width),
            total_trip_time.to_string().draw_text(ctx),
        ]));

        let (_, waiting) = app.primary.sim.finished_trip_time(id).unwrap();
        col.push(Widget::custom_row(vec![
            Widget::custom_row(vec![Line("Total waiting time").secondary().draw(ctx)])
                .force_width_pct(ctx, col_width),
            waiting.to_string().draw_text(ctx),
        ]));

        col.push(Widget::custom_row(vec![
            Widget::custom_row(vec![Line("Purpose").secondary().draw(ctx)])
                .force_width_pct(ctx, col_width),
            Line(trip.purpose.to_string()).secondary().draw(ctx),
        ]));
    }

    col.push(make_timeline(
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

pub fn cancelled(ctx: &mut EventCtx, app: &App, id: TripID) -> Widget {
    let trip = app.primary.sim.trip_info(id);

    let mut col = vec![Text::from(Line(format!(
        "Trip cancelled: {}",
        trip.cancellation_reason.as_ref().unwrap()
    )))
    .wrap_to_pct(ctx, 20)
    .draw(ctx)];

    // TODO Warp buttons. make_table is showing its age.
    let (_, _, name1) = endpoint(&trip.start, app);
    let (_, _, name2) = endpoint(&trip.end, app);
    col.extend(make_table(
        ctx,
        vec![
            ("Departure", trip.departure.ampm_tostring()),
            ("From", name1),
            ("To", name2),
            ("Purpose", trip.purpose.to_string()),
        ],
    ));

    Widget::col(col)
}

/// Highlights intersections which were "slow" on the map
fn highlight_slow_intersections(ctx: &EventCtx, app: &App, details: &mut Details, id: TripID) {
    let intersection_delays = &app
        .primary
        .sim
        .get_analytics()
        .trip_intersection_delays
        .get(&id);
    if let Some(intersection_delays) = intersection_delays {
        for (id, time) in intersection_delays.iter() {
            let intersection = app.primary.map.get_i(id.parent);
            // Maybe alter the delay times
            let (normal_delay_time, slow_delay_time) = if intersection.is_traffic_signal() {
                (30, 120)
            } else {
                (5, 30)
            };
            let (fg_color, bg_color) = if *time < normal_delay_time {
                (Color::WHITE, app.cs.normal_slow_intersection)
            } else if *time < slow_delay_time {
                (Color::BLACK, app.cs.slow_intersection)
            } else {
                (Color::WHITE, app.cs.very_slow_intersection)
            };

            let time_duration = Duration::seconds(*time as f64);
            details.unzoomed.append(
                Text::from(TextSpan::from(
                    Line(format!("{}", time_duration)).fg(fg_color),
                ))
                .bg(bg_color)
                .render(ctx)
                .centered_on(intersection.polygon.center()),
            );
            details.zoomed.append(
                Text::from(TextSpan::from(
                    Line(format!("{}", time_duration)).fg(fg_color),
                ))
                .bg(bg_color)
                .render(ctx)
                .scale(0.4)
                .centered_on(intersection.polygon.center()),
            );
        }
    }
}

/// Highlights lanes which were "slow" on the map
fn highlight_slow_lanes(ctx: &EventCtx, app: &App, details: &mut Details, id: TripID) {
    if let Some(lane_speeds) = &app
        .primary
        .sim
        .get_analytics()
        .lane_speed_percentage
        .get(&id)
    {
        for (id, speed_percent) in lane_speeds.iter() {
            let lane = app.primary.map.get_l(*id);
            let (fg_color, bg_color) = if speed_percent > &95 {
                (Color::WHITE, app.cs.normal_slow_intersection)
            } else if speed_percent > &60 {
                (Color::BLACK, app.cs.slow_intersection)
            } else {
                (Color::WHITE, app.cs.very_slow_intersection)
            };
            details.unzoomed.push(
                bg_color,
                lane.lane_center_pts.make_polygons(Distance::meters(10.0)),
            );
            details.zoomed.extend(
                bg_color,
                lane.lane_center_pts.dashed_lines(
                    Distance::meters(0.75),
                    Distance::meters(1.0),
                    Distance::meters(0.4),
                ),
            );
            let (pt, _) = lane
                .lane_center_pts
                .must_dist_along(lane.lane_center_pts.length() / 2.0);
            details.unzoomed.append(
                Text::from(TextSpan::from(
                    Line(format!("{}s", speed_percent)).fg(fg_color),
                ))
                .bg(bg_color)
                .render(ctx)
                .centered_on(pt),
            );
            details.zoomed.append(
                Text::from(TextSpan::from(
                    Line(format!("{}s", speed_percent)).fg(fg_color),
                ))
                .bg(bg_color)
                .render(ctx)
                .scale(0.4)
                .centered_on(pt),
            );
        }
    }
}

/// Helper func for make_bar()
/// Builds a default text overlay widget
fn build_text(msgs: &Vec<String>, distance: &Distance) -> Text {
    let mut display_txt = Text::new();
    for msg in msgs {
        display_txt.add(Line(msg));
    }
    display_txt.add(Line(format!(
        "  Distance covered: {}",
        distance.describe_rounded()
    )));
    display_txt
}

/// Makes the timeline bar in trip info panel
fn make_bar(
    ctx: &mut EventCtx,
    app: &App,
    trip_id: TripID,
    phases: &Vec<TripPhase>,
    progress_along_path: Option<f64>,
) -> Widget {
    let map = &app.primary.map;
    let blank_lane_speeds_bt = BTreeMap::new();
    let blank_intersection_delays_bt = BTreeMap::new();
    let intersection_delays = &app
        .primary
        .sim
        .get_analytics()
        .trip_intersection_delays
        .get(&trip_id)
        .unwrap_or(&blank_intersection_delays_bt);
    let lane_speeds = &app
        .primary
        .sim
        .get_analytics()
        .lane_speed_percentage
        .get(&trip_id)
        .unwrap_or(&blank_lane_speeds_bt);
    let box_width = 0.22 * ctx.canvas.window_width;
    let mut total_dist = Distance::meters(0.0);
    let mut segments: Vec<(Distance, Color, Text)> = Vec::new();
    let trip = app.primary.sim.trip_info(trip_id);
    let end_time = phases.last().as_ref().and_then(|p| p.end_time);
    let total_duration_so_far = end_time.unwrap_or_else(|| app.primary.sim.time()) - trip.departure;

    let mut icons_geom = Vec::new();
    let mut icon_pos = Vec::new();
    let mut icons = Vec::new();
    for (idx, p) in phases.into_iter().enumerate() {
        let mut msgs = Vec::new();
        let norm_color = color_for_trip_phase(app, p.phase_type).alpha(0.7);
        msgs.push(p.phase_type.describe(map));
        msgs.push(format!("  Started at {}", p.start_time.ampm_tostring()));
        let phase_duration = if let Some(t2) = p.end_time {
            let d = t2 - p.start_time;
            msgs.push(format!(
                "  Ended at {} (duration: {})",
                t2.ampm_tostring(),
                d
            ));
            d
        } else {
            let d = app.primary.sim.time() - p.start_time;
            msgs.push(format!("  Ongoing (duration so far: {})", d));
            d
        };
        // TODO Problems when this is really low?
        let percent_duration = if total_duration_so_far == Duration::ZERO {
            0.0
        } else {
            phase_duration / total_duration_so_far
        };

        let phase_width = box_width * percent_duration;
        if idx == phases.len() - 1 {
            if let Some(p) = progress_along_path {
                icons.push(Widget::draw_batch(
                    ctx,
                    GeomBatch::load_svg(ctx.prerender, "system/assets/timeline/current_pos.svg")
                        .centered_on(Pt2D::new(p * phase_width, 7.5)),
                ));
            }
        }
        icons_geom.push(GeomBatch::load_svg(
            ctx.prerender,
            match p.phase_type {
                TripPhaseType::Driving => "system/assets/timeline/driving.svg",
                TripPhaseType::Walking => "system/assets/timeline/walking.svg",
                TripPhaseType::Biking => "system/assets/timeline/biking.svg",
                TripPhaseType::Parking => "system/assets/timeline/parking.svg",
                TripPhaseType::WaitingForBus(_, _) => "system/assets/timeline/waiting_for_bus.svg",
                TripPhaseType::RidingBus(_, _, _) => "system/assets/timeline/riding_bus.svg",
                TripPhaseType::Cancelled | TripPhaseType::Finished => unreachable!(),
                TripPhaseType::DelayedStart => "system/assets/timeline/delayed_start.svg",
                // TODO What icon should represent this?
                TripPhaseType::Remote => "system/assets/timeline/delayed_start.svg",
            },
        ));
        msgs.push(format!(
            "  {}% of trip percentage",
            (100.0 * percent_duration) as usize
        ));

        msgs.push(format!(
            "  Total delayed time {}",
            app.primary.sim.trip_blocked_time(trip_id)
        ));
        let mut sum_phase_dist = Distance::ZERO;
        if let Some((_, step_list)) = &p.path {
            let mut norm_distance = Distance::meters(0.0);
            for step in step_list.get_steps() {
                match step {
                    PathStep::Lane(id) | PathStep::ContraflowLane(id) => {
                        let lane_detail = map.get_l(*id);
                        sum_phase_dist += lane_detail.length();
                        if let Some(avg_speed) = lane_speeds.get(id) {
                            segments.push((
                                norm_distance,
                                norm_color,
                                build_text(&msgs, &norm_distance),
                            ));
                            let mut display_txt = Text::from(Line(&p.phase_type.describe(map)));
                            display_txt.add(Line(format!(
                                "  Road: {}",
                                map.get_r(lane_detail.parent)
                                    .get_name(app.opts.language.as_ref())
                            )));
                            display_txt.add(Line(format!("  Lane ID: {}", id)));
                            display_txt.add(Line(format!(
                                "  Lane distance: {}",
                                lane_detail.length().describe_rounded()
                            )));
                            display_txt.add(Line(format!("  Average speed: {}", avg_speed)));
                            segments.push((lane_detail.length(), Color::RED, display_txt));
                            norm_distance = Distance::meters(0.0);
                        } else {
                            norm_distance += lane_detail.length();
                        }
                    }
                    PathStep::Turn(id) => {
                        let turn_details = map.get_t(*id);
                        sum_phase_dist += turn_details.geom.length();
                        if let Some(delay) = intersection_delays.get(id) {
                            segments.push((
                                norm_distance,
                                norm_color,
                                build_text(&msgs, &norm_distance),
                            ));

                            let mut display_txt = Text::from(Line(&p.phase_type.describe(map)));
                            display_txt.add(Line(format!("  Intersection: {}", id.parent)));
                            display_txt.add(Line(format!(
                                "  Delay: {}",
                                Duration::seconds(*delay as f64)
                            )));
                            segments.push((
                                // To make sure that the hotspot isn't too small
                                if 0.05 < (turn_details.geom.length() / sum_phase_dist) {
                                    turn_details.geom.length()
                                } else {
                                    sum_phase_dist += sum_phase_dist * 0.05;
                                    sum_phase_dist -= turn_details.geom.length();
                                    sum_phase_dist * 0.05
                                },
                                Color::RED,
                                display_txt,
                            ));
                            norm_distance = Distance::meters(0.0);
                        } else {
                            norm_distance += turn_details.geom.length();
                        }
                    }
                }
            }
            segments.push((norm_distance, norm_color, build_text(&msgs, &norm_distance)));
        } else {
            // TODO Think of something to do instead
            error!("No path for {}", trip_id)
        }
        icon_pos.push(sum_phase_dist / 2.0);
        total_dist += sum_phase_dist;
    }
    let mut timeline = Vec::new();
    assert_eq!(icons_geom.len(), icon_pos.len());
    let mut offset = 0.0;
    for index in 0..icon_pos.len() {
        let pos = box_width * (*icon_pos.get(index).unwrap() / total_dist);
        let img = icons_geom.get(index).unwrap().to_owned();
        let img_size = img.get_dims();
        icons.push(Widget::draw_batch(
            ctx,
            img.centered_on(
                // TODO Hardcoded layouting...
                Pt2D::new(pos + offset, 15.0),
            ),
        ));
        offset += (pos * 2.0) - img_size.width;
    }
    for seg in segments {
        let seg_percent = seg.0 / total_dist;
        let seg_with = seg_percent * box_width;
        let rect = Polygon::rectangle(seg_with, 15.0);
        let batch = GeomBatch::from(vec![(seg.1, rect.clone())]);
        timeline.push(
            DrawWithTooltips::new(
                ctx,
                batch,
                vec![(rect, seg.2)],
                Box::new(|_| GeomBatch::new()),
            )
            .centered_vert(),
        );
    }
    Widget::custom_col(vec![
        Widget::custom_row(icons),
        Widget::custom_row(timeline),
    ])
}

/// Builds the timeline widget
/// And draws the route on the map
fn make_timeline(
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
        if let TripEndpoint::Border(_, ref loc) = trip.start {
            if let Some(loc) = loc {
                if let Ok(pl) =
                    PolyLine::new(vec![Pt2D::from_gps(loc.gps, map.get_gps_bounds()), center])
                {
                    let arrow = pl.make_arrow(Distance::meters(5.0), ArrowCap::Triangle);
                    details.unzoomed.push(Color::GREEN, arrow.clone());
                    details.zoomed.push(Color::GREEN, arrow.clone());
                }
            }
        }

        details.unzoomed.append(
            GeomBatch::load_svg(ctx.prerender, "system/assets/timeline/start_pos.svg")
                .scale(3.0)
                .color(RewriteColor::Change(Color::WHITE, Color::BLACK))
                .color(RewriteColor::Change(
                    Color::hex("#5B5B5B"),
                    Color::hex("#CC4121"),
                ))
                .centered_on(center),
        );
        details.zoomed.append(
            GeomBatch::load_svg(ctx.prerender, "system/assets/timeline/start_pos.svg")
                .color(RewriteColor::Change(Color::WHITE, Color::BLACK))
                .color(RewriteColor::Change(
                    Color::hex("#5B5B5B"),
                    Color::hex("#CC4121"),
                ))
                .centered_on(center),
        );

        Btn::svg(
            "system/assets/timeline/start_pos.svg",
            RewriteColor::Change(Color::WHITE, app.cs.hovering),
        )
        .tooltip(Text::from(Line(name)))
        .build(ctx, format!("jump to start of {}", trip_id), None)
    };

    let goal_btn = {
        let (id, center, name) = endpoint(&trip.end, app);
        details
            .warpers
            .insert(format!("jump to goal of {}", trip_id), id);
        if let TripEndpoint::Border(_, ref loc) = trip.end {
            if let Some(loc) = loc {
                if let Ok(pl) =
                    PolyLine::new(vec![center, Pt2D::from_gps(loc.gps, map.get_gps_bounds())])
                {
                    let arrow = pl.make_arrow(Distance::meters(5.0), ArrowCap::Triangle);
                    details.unzoomed.push(Color::GREEN, arrow.clone());
                    details.zoomed.push(Color::GREEN, arrow.clone());
                }
            }
        }

        details.unzoomed.append(
            GeomBatch::load_svg(ctx.prerender, "system/assets/timeline/goal_pos.svg")
                .scale(3.0)
                .color(RewriteColor::Change(Color::WHITE, Color::BLACK))
                .color(RewriteColor::Change(
                    Color::hex("#5B5B5B"),
                    Color::hex("#CC4121"),
                ))
                .centered_on(center),
        );
        details.zoomed.append(
            GeomBatch::load_svg(ctx.prerender, "system/assets/timeline/goal_pos.svg")
                .color(RewriteColor::Change(Color::WHITE, Color::BLACK))
                .color(RewriteColor::Change(
                    Color::hex("#5B5B5B"),
                    Color::hex("#CC4121"),
                ))
                .centered_on(center),
        );

        Btn::svg(
            "system/assets/timeline/goal_pos.svg",
            RewriteColor::Change(Color::WHITE, app.cs.hovering),
        )
        .tooltip(Text::from(Line(name)))
        .build(ctx, format!("jump to goal of {}", trip_id), None)
    };

    let timeline = make_bar(ctx, app, trip_id, &phases, progress_along_path);
    let mut elevation = Vec::new();
    let mut path_impossible = false;
    for (idx, p) in phases.into_iter().enumerate() {
        let color = color_for_trip_phase(app, p.phase_type).alpha(0.7);
        if let Some((dist, ref path)) = p.path {
            if app.opts.dev
                && (p.phase_type == TripPhaseType::Walking || p.phase_type == TripPhaseType::Biking)
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
                if let Some(trace) = path.trace(map_for_pathfinding, dist, None) {
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
        Widget::custom_row(vec![start_btn, timeline, goal_btn])
            .evenly_spaced()
            .margin_above(25),
        Widget::row(vec![
            trip.departure.ampm_tostring().draw_text(ctx),
            if let Some(t) = end_time {
                t.ampm_tostring().draw_text(ctx).align_right()
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
                Btn::svg(
                    "system/assets/speed/info_jump_to_time.svg",
                    RewriteColor::Change(Color::WHITE, app.cs.hovering),
                )
                .tooltip({
                    let mut txt = Text::from(Line("This will jump to "));
                    txt.append(Line(trip.departure.ampm_tostring()).fg(Color::hex("#F9EC51")));
                    txt.add(Line("The simulation will continue, and your score"));
                    txt.add(Line("will be calculated at this new time."));
                    txt
                })
                .build(ctx, format!("jump to {}", trip.departure), None)
            } else {
                Widget::nothing()
            },
            if let Some(t) = end_time {
                if details.can_jump_to_time {
                    details
                        .time_warpers
                        .insert(format!("jump to {}", t), (trip_id, t));
                    Btn::svg(
                        "system/assets/speed/info_jump_to_time.svg",
                        RewriteColor::Change(Color::WHITE, app.cs.hovering),
                    )
                    .tooltip({
                        let mut txt = Text::from(Line("This will jump to "));
                        txt.append(Line(t.ampm_tostring()).fg(Color::hex("#F9EC51")));
                        txt.add(Line("The simulation will continue, and your score"));
                        txt.add(Line("will be calculated at this new time."));
                        txt
                    })
                    .build(ctx, format!("jump to {}", t), None)
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
        col.push("Map edits have disconnected the path taken before".draw_text(ctx));
    }
    // TODO This just needs too much more work
    if false {
        col.extend(elevation);
    }
    highlight_slow_intersections(ctx, app, details, trip_id);
    highlight_slow_lanes(ctx, app, details, trip_id);
    Widget::col(col)
}

fn make_elevation(ctx: &EventCtx, color: Color, walking: bool, path: &Path, map: &Map) -> Widget {
    let mut pts: Vec<(Distance, Distance)> = Vec::new();
    let mut dist = Distance::ZERO;
    for step in path.get_steps() {
        if let PathStep::Turn(t) = step {
            pts.push((dist, map.get_i(t.parent).elevation));
        }
        dist += step.as_traversable().length(map);
    }
    // TODO Plot needs to support Distance as both X and Y axis. :P
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
            pts: pts
                .into_iter()
                .map(|(x, y)| {
                    (
                        Time::START_OF_DAY + Duration::seconds(x.inner_meters()),
                        y.inner_meters() as usize,
                    )
                })
                .collect(),
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
        TripEndpoint::Border(i, _) => {
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
    }
}
