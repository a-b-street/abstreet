use crate::app::App;
use crate::helpers::{color_for_trip_phase, ID};
use crate::info::{make_table, Details, Tab};
use ezgui::{
    Btn, Color, EventCtx, GeomBatch, Line, LinePlot, PlotOptions, RewriteColor, Series, Text,
    TextExt, Widget,
};
use geom::{Angle, ArrowCap, Distance, Duration, PolyLine, Polygon, Pt2D, Time};
use map_model::{Map, Path, PathStep};
use maplit::btreemap;
use sim::{AgentID, PersonID, TripEndpoint, TripID, TripPhase, TripPhaseType, VehicleType};
use std::collections::BTreeMap;

#[derive(Clone)]
pub struct OpenTrip {
    pub show_after: bool,
    // (unzoomed, zoomed)
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
    trip: TripID,
    agent: AgentID,
    open_trip: &mut OpenTrip,
    details: &mut Details,
) -> Widget {
    let phases = app
        .primary
        .sim
        .get_analytics()
        .get_trip_phases(trip, &app.primary.map);
    let (start_time, _, _, _) = app.primary.sim.trip_info(trip);

    let col_width = 7;
    let props = app.primary.sim.agent_properties(agent);
    // This is different than the entire TripMode, and also not the current TripPhaseType.
    // Sigh.
    let activity = match agent {
        AgentID::Pedestrian(_) => "walking",
        AgentID::Car(c) => match c.1 {
            VehicleType::Car => "driving",
            VehicleType::Bike => "biking",
            // TODO And probably riding a bus is broken, I don't know how that gets mapped right
            // now
            VehicleType::Bus => "riding the bus",
        },
    };
    let time_so_far = app.primary.sim.time() - start_time;

    let mut col = Vec::new();

    {
        col.push(Widget::row(vec![
            Widget::row(vec![Line("Trip time").secondary().draw(ctx)])
                .force_width_pct(ctx, col_width),
            Text::from_all(vec![
                Line(props.total_time.to_string()),
                Line(format!(" {} / {} this trip", activity, time_so_far)).secondary(),
            ])
            .draw(ctx),
        ]));
    }
    {
        col.push(Widget::row(vec![
            Widget::row(vec![Line("Distance").secondary().draw(ctx)])
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
        col.push(Widget::row(vec![
            Widget::row(vec![Line("Waiting").secondary().draw(ctx)])
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

    col.push(make_timeline(
        ctx,
        app,
        trip,
        open_trip,
        details,
        phases,
        Some(props.dist_crossed / props.total_dist),
    ));

    Widget::col(col)
}

pub fn future(
    ctx: &mut EventCtx,
    app: &App,
    trip: TripID,
    open_trip: &mut OpenTrip,
    details: &mut Details,
) -> Widget {
    let (start_time, trip_start, trip_end, _) = app.primary.sim.trip_info(trip);

    let mut col = Vec::new();

    let now = app.primary.sim.time();
    if now > start_time {
        col.extend(make_table(
            ctx,
            vec![("Start delayed", (now - start_time).to_string())],
        ));
    }

    if let Some(estimated_trip_time) = app
        .has_prebaked()
        .and_then(|_| app.prebaked().finished_trip_time(trip))
    {
        col.extend(make_table(
            ctx,
            vec![("Estimated trip time", estimated_trip_time.to_string())],
        ));

        let phases = app.prebaked().get_trip_phases(trip, &app.primary.map);
        col.push(make_timeline(
            ctx, app, trip, open_trip, details, phases, None,
        ));
    } else {
        // TODO Warp buttons. make_table is showing its age.
        let (_, _, name1) = endpoint(&trip_start, &app.primary.map);
        let (_, _, name2) = endpoint(&trip_end, &app.primary.map);
        col.extend(make_table(
            ctx,
            vec![
                ("Departure", start_time.ampm_tostring()),
                ("From", name1),
                ("To", name2),
            ],
        ));

        col.push(
            Btn::text_bg2("Wait for trip")
                .tooltip(Text::from(Line(format!(
                    "This will advance the simulation to {}",
                    start_time.ampm_tostring()
                ))))
                .build(ctx, format!("wait for {}", trip), None)
                .margin(5),
        );
        details
            .time_warpers
            .insert(format!("wait for {}", trip), (trip, start_time));
    }

    Widget::col(col)
}

pub fn finished(
    ctx: &mut EventCtx,
    app: &App,
    person: PersonID,
    open_trips: &mut BTreeMap<TripID, OpenTrip>,
    trip: TripID,
    details: &mut Details,
) -> Widget {
    let (start_time, _, _, _) = app.primary.sim.trip_info(trip);
    let phases = if open_trips[&trip].show_after {
        app.primary
            .sim
            .get_analytics()
            .get_trip_phases(trip, &app.primary.map)
    } else {
        app.prebaked().get_trip_phases(trip, &app.primary.map)
    };

    let mut col = Vec::new();

    if open_trips[&trip].show_after && app.has_prebaked().is_some() {
        let mut open = open_trips.clone();
        open.insert(
            trip,
            OpenTrip {
                show_after: false,
                cached_routes: Vec::new(),
            },
        );
        details.hyperlinks.insert(
            format!("show before changes for {}", trip),
            Tab::PersonTrips(person, open),
        );
        col.push(
            Btn::text_bg(
                format!("show before changes for {}", trip),
                Text::from_all(vec![Line("After / "), Line("Before").secondary()]),
                app.cs.section_bg,
                app.cs.hovering,
            )
            .build_def(ctx, None),
        );
    } else if app.has_prebaked().is_some() {
        let mut open = open_trips.clone();
        open.insert(trip, OpenTrip::new());
        details.hyperlinks.insert(
            format!("show after changes for {}", trip),
            Tab::PersonTrips(person, open),
        );
        col.push(
            Btn::text_bg(
                format!("show after changes for {}", trip),
                Text::from_all(vec![Line("After / ").secondary(), Line("Before")]),
                app.cs.section_bg,
                app.cs.hovering,
            )
            .build_def(ctx, None),
        );
    }

    {
        let col_width = 15;

        let total_trip_time = phases.last().as_ref().and_then(|p| p.end_time).unwrap() - start_time;
        col.push(Widget::row(vec![
            Widget::row(vec![Line("Trip time").secondary().draw(ctx)])
                .force_width_pct(ctx, col_width),
            total_trip_time.to_string().draw_text(ctx),
        ]));

        let (_, waiting) = app.primary.sim.finished_trip_time(trip).unwrap();
        col.push(Widget::row(vec![
            Widget::row(vec![Line("Total waiting time").secondary().draw(ctx)])
                .force_width_pct(ctx, col_width),
            waiting.to_string().draw_text(ctx),
        ]));
    }

    col.push(make_timeline(
        ctx,
        app,
        trip,
        open_trips.get_mut(&trip).unwrap(),
        details,
        phases,
        None,
    ));

    Widget::col(col)
}

pub fn aborted(ctx: &mut EventCtx, app: &App, trip: TripID) -> Widget {
    let (start_time, trip_start, trip_end, _) = app.primary.sim.trip_info(trip);

    let mut col = vec![Text::from_multiline(vec![
        Line("A glitch in the simulation happened."),
        Line("This trip, however, did not."),
    ])
    .draw(ctx)];

    // TODO Warp buttons. make_table is showing its age.
    let (_, _, name1) = endpoint(&trip_start, &app.primary.map);
    let (_, _, name2) = endpoint(&trip_end, &app.primary.map);
    col.extend(make_table(
        ctx,
        vec![
            ("Departure", start_time.ampm_tostring()),
            ("From", name1),
            ("To", name2),
        ],
    ));

    Widget::col(col)
}

fn make_timeline(
    ctx: &mut EventCtx,
    app: &App,
    trip: TripID,
    open_trip: &mut OpenTrip,
    details: &mut Details,
    phases: Vec<TripPhase>,
    progress_along_path: Option<f64>,
) -> Widget {
    let map = &app.primary.map;
    let sim = &app.primary.sim;
    // TODO Repeating stuff
    let (start_time, trip_start, trip_end, _) = sim.trip_info(trip);
    let end_time = phases.last().as_ref().and_then(|p| p.end_time);

    let start_btn = {
        let (id, center, name) = endpoint(&trip_start, map);
        details
            .warpers
            .insert(format!("jump to start of {}", trip), id);
        if let TripEndpoint::Border(_, ref loc) = trip_start {
            if let Some(loc) = loc {
                let arrow = PolyLine::new(vec![
                    Pt2D::forcibly_from_gps(loc.gps, map.get_gps_bounds()),
                    center,
                ])
                .make_arrow(Distance::meters(5.0), ArrowCap::Triangle)
                .unwrap();
                details.unzoomed.push(Color::GREEN, arrow.clone());
                details.zoomed.push(Color::GREEN, arrow.clone());
            }
        }

        details.unzoomed.add_svg(
            ctx.prerender,
            "../data/system/assets/timeline/start_pos.svg",
            center,
            1.0,
            Angle::ZERO,
            RewriteColor::NoOp,
            true,
        );
        details.zoomed.add_svg(
            ctx.prerender,
            "../data/system/assets/timeline/start_pos.svg",
            center,
            0.5,
            Angle::ZERO,
            RewriteColor::NoOp,
            true,
        );

        Btn::svg(
            "../data/system/assets/timeline/start_pos.svg",
            RewriteColor::Change(Color::WHITE, app.cs.hovering),
        )
        .tooltip(Text::from(Line(name)))
        .build(ctx, format!("jump to start of {}", trip), None)
    };

    let goal_btn = {
        let (id, center, name) = endpoint(&trip_end, map);
        details
            .warpers
            .insert(format!("jump to goal of {}", trip), id);
        if let TripEndpoint::Border(_, ref loc) = trip_end {
            if let Some(loc) = loc {
                let arrow = PolyLine::new(vec![
                    center,
                    Pt2D::forcibly_from_gps(loc.gps, map.get_gps_bounds()),
                ])
                .make_arrow(Distance::meters(5.0), ArrowCap::Triangle)
                .unwrap();
                details.unzoomed.push(Color::GREEN, arrow.clone());
                details.zoomed.push(Color::GREEN, arrow.clone());
            }
        }

        details.unzoomed.add_svg(
            ctx.prerender,
            "../data/system/assets/timeline/goal_pos.svg",
            center,
            1.0,
            Angle::ZERO,
            RewriteColor::NoOp,
            true,
        );
        details.zoomed.add_svg(
            ctx.prerender,
            "../data/system/assets/timeline/goal_pos.svg",
            center,
            0.5,
            Angle::ZERO,
            RewriteColor::NoOp,
            true,
        );

        Btn::svg(
            "../data/system/assets/timeline/goal_pos.svg",
            RewriteColor::Change(Color::WHITE, app.cs.hovering),
        )
        .tooltip(Text::from(Line(name)))
        .build(ctx, format!("jump to goal of {}", trip), None)
    };

    let total_duration_so_far = end_time.unwrap_or_else(|| sim.time()) - phases[0].start_time;

    let total_width = 0.22 * ctx.canvas.window_width / ctx.get_scale_factor();
    let mut timeline = Vec::new();
    let num_phases = phases.len();
    let mut elevation = Vec::new();
    let mut path_impossible = false;
    for (idx, p) in phases.into_iter().enumerate() {
        let color = color_for_trip_phase(app, p.phase_type).alpha(0.7);

        let mut txt = Text::from(Line(&p.phase_type.describe(map)));
        txt.add(Line(format!(
            "- Started at {}",
            p.start_time.ampm_tostring()
        )));
        let duration = if let Some(t2) = p.end_time {
            let d = t2 - p.start_time;
            txt.add(Line(format!(
                "- Ended at {} (duration: {})",
                t2.ampm_tostring(),
                d
            )));
            d
        } else {
            let d = sim.time() - p.start_time;
            txt.add(Line(format!("- Ongoing (duration so far: {})", d)));
            d
        };
        // TODO Problems when this is really low?
        let percent_duration = if total_duration_so_far == Duration::ZERO {
            0.0
        } else {
            duration / total_duration_so_far
        };
        txt.add(Line(format!(
            "- {}% of trip duration",
            (100.0 * percent_duration) as usize
        )));

        let phase_width = total_width * percent_duration;
        let rect = Polygon::rectangle(phase_width, 15.0);
        let mut normal = GeomBatch::from(vec![(color, rect.clone())]);
        if idx == num_phases - 1 {
            if let Some(p) = progress_along_path {
                normal.add_svg(
                    ctx.prerender,
                    "../data/system/assets/timeline/current_pos.svg",
                    Pt2D::new(p * phase_width, 7.5 * ctx.get_scale_factor()),
                    1.0,
                    Angle::ZERO,
                    RewriteColor::NoOp,
                    false,
                );
            }
        }
        normal.add_svg(
            ctx.prerender,
            match p.phase_type {
                TripPhaseType::Driving => "../data/system/assets/timeline/driving.svg",
                TripPhaseType::Walking => "../data/system/assets/timeline/walking.svg",
                TripPhaseType::Biking => "../data/system/assets/timeline/biking.svg",
                TripPhaseType::Parking => "../data/system/assets/timeline/parking.svg",
                TripPhaseType::WaitingForBus(_, _) => {
                    "../data/system/assets/timeline/waiting_for_bus.svg"
                }
                TripPhaseType::RidingBus(_, _, _) => {
                    "../data/system/assets/timeline/riding_bus.svg"
                }
                TripPhaseType::Aborted | TripPhaseType::Finished => unreachable!(),
                TripPhaseType::DelayedStart => "../data/system/assets/timeline/delayed_start.svg",
                // TODO What icon should represent this?
                TripPhaseType::Remote => "../data/system/assets/timeline/delayed_start.svg",
            },
            // TODO Hardcoded layouting...
            Pt2D::new(0.5 * phase_width, -20.0 * ctx.get_scale_factor()),
            1.0,
            Angle::ZERO,
            RewriteColor::NoOp,
            false,
        );

        let mut hovered = GeomBatch::from(vec![(color.alpha(1.0), rect.clone())]);
        for (c, p) in normal.clone().consume().into_iter().skip(1) {
            hovered.fancy_push(c, p);
        }

        timeline.push(
            Btn::custom(normal, hovered, rect)
                .tooltip(txt)
                .build(
                    ctx,
                    format!("examine trip phase {} of {}", idx + 1, trip),
                    None,
                )
                .centered_vert(),
        );

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
                if let Some(trace) = path.trace(map, dist, None) {
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
    }

    timeline.insert(0, start_btn.margin(5));
    timeline.push(goal_btn.margin(5));

    let mut col = vec![
        Widget::row(timeline).evenly_spaced().margin_above(25),
        Widget::row(vec![
            start_time.ampm_tostring().draw_text(ctx),
            if let Some(t) = end_time {
                t.ampm_tostring().draw_text(ctx).align_right()
            } else {
                Widget::nothing()
            },
        ]),
        Widget::row(vec![
            {
                details
                    .time_warpers
                    .insert(format!("jump to {}", start_time), (trip, start_time));
                Btn::svg(
                    "../data/system/assets/speed/info_jump_to_time.svg",
                    RewriteColor::Change(Color::WHITE, app.cs.hovering),
                )
                .tooltip({
                    let mut txt = Text::from(Line("This will jump to "));
                    txt.append(Line(start_time.ampm_tostring()).fg(Color::hex("#F9EC51")));
                    txt.add(Line("The simulation will continue, and your score"));
                    txt.add(Line("will be calculated at this new time."));
                    txt
                })
                .build(ctx, format!("jump to {}", start_time), None)
            },
            if let Some(t) = end_time {
                details
                    .time_warpers
                    .insert(format!("jump to {}", t), (trip, t));
                Btn::svg(
                    "../data/system/assets/speed/info_jump_to_time.svg",
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
            },
        ])
        .margin_above(5),
    ];
    if path_impossible {
        col.push("Map edits have disconnected the path taken before".draw_text(ctx));
    }
    // TODO This just needs too much more work
    if false {
        col.extend(elevation);
    }
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
        "elevation",
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
        PlotOptions::new(),
    )
}

// (ID, center, name)
fn endpoint(endpt: &TripEndpoint, map: &Map) -> (ID, Pt2D, String) {
    match endpt {
        TripEndpoint::Bldg(b) => {
            let bldg = map.get_b(*b);
            (ID::Building(*b), bldg.label_center, bldg.address.clone())
        }
        TripEndpoint::Border(i, _) => {
            let i = map.get_i(*i);
            (
                ID::Intersection(i.id),
                i.polygon.center(),
                format!("off map, via {}", i.name(map)),
            )
        }
    }
}
