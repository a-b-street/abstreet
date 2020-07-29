use crate::app::App;
use crate::helpers::{color_for_trip_phase, ID};
use crate::info::{make_table, Details, Tab};
use ezgui::{
    Btn, Color, EventCtx, GeomBatch, Line, LinePlot, PlotOptions, RewriteColor, Series, Text,
    TextExt, Widget,
};
use geom::{ArrowCap, Distance, Duration, PolyLine, Polygon, Pt2D, Time};
use map_model::{Map, Path, PathStep};
use maplit::btreemap;
use sim::{AgentID, PersonID, TripEndpoint, TripID, TripPhase, TripPhaseType};
use std::collections::BTreeMap;

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

    let col_width = 7;
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

    col.push(make_timeline(
        ctx,
        app,
        id,
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
            vec![("Start delayed", (now - trip.departure).to_string())].into_iter(),
        ));
    }

    if let Some(estimated_trip_time) = app
        .has_prebaked()
        .and_then(|_| app.prebaked().finished_trip_time(id))
    {
        col.extend(make_table(
            ctx,
            vec![("Estimated trip time", estimated_trip_time.to_string())].into_iter(),
        ));

        let phases = app.prebaked().get_trip_phases(id, &app.primary.map);
        col.push(make_timeline(
            ctx, app, id, open_trip, details, phases, None,
        ));
    } else {
        // TODO Warp buttons. make_table is showing its age.
        let (id1, _, name1) = endpoint(&trip.start, &app.primary.map);
        let (id2, _, name2) = endpoint(&trip.end, &app.primary.map);
        details
            .warpers
            .insert(format!("jump to start of {}", id), id1);
        details
            .warpers
            .insert(format!("jump to goal of {}", id), id2);
        details
            .time_warpers
            .insert(format!("wait for {}", id), (id, trip.departure));

        col.push(
            Widget::row(vec![
                Btn::svg(
                    "system/assets/timeline/start_pos.svg",
                    RewriteColor::Change(Color::WHITE, app.cs.hovering),
                )
                .tooltip(Text::from(Line(name1)))
                .build(ctx, format!("jump to start of {}", id), None),
                Btn::text_bg2("Wait for trip")
                    .tooltip(Text::from(Line(format!(
                        "This will advance the simulation to {}",
                        trip.departure.ampm_tostring()
                    ))))
                    .build(ctx, format!("wait for {}", id), None),
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
            vec![("Departure", trip.departure.ampm_tostring())].into_iter(),
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
    let trip = app.primary.sim.trip_info(id);
    let phases = if open_trips[&id].show_after {
        app.primary
            .sim
            .get_analytics()
            .get_trip_phases(id, &app.primary.map)
    } else {
        app.prebaked().get_trip_phases(id, &app.primary.map)
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
        let col_width = 15;

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
    }

    col.push(make_timeline(
        ctx,
        app,
        id,
        open_trips.get_mut(&id).unwrap(),
        details,
        phases,
        None,
    ));

    Widget::col(col)
}

pub fn aborted(ctx: &mut EventCtx, app: &App, id: TripID) -> Widget {
    let trip = app.primary.sim.trip_info(id);

    let mut col = vec![Text::from_multiline(vec![
        Line("A glitch in the simulation happened."),
        Line("This trip, however, did not."),
    ])
    .draw(ctx)];

    // TODO Warp buttons. make_table is showing its age.
    let (_, _, name1) = endpoint(&trip.start, &app.primary.map);
    let (_, _, name2) = endpoint(&trip.end, &app.primary.map);
    col.extend(make_table(
        ctx,
        vec![
            ("Departure", trip.departure.ampm_tostring()),
            ("From", name1),
            ("To", name2),
        ]
        .into_iter(),
    ));

    Widget::col(col)
}

pub fn cancelled(ctx: &mut EventCtx, app: &App, id: TripID) -> Widget {
    let trip = app.primary.sim.trip_info(id);

    let mut col = vec!["Trip cancelled due to traffic pattern modifications".draw_text(ctx)];

    // TODO Warp buttons. make_table is showing its age.
    let (_, _, name1) = endpoint(&trip.start, &app.primary.map);
    let (_, _, name2) = endpoint(&trip.end, &app.primary.map);
    col.extend(make_table(
        ctx,
        vec![
            ("Departure", trip.departure.ampm_tostring()),
            ("From", name1),
            ("To", name2),
        ]
        .into_iter(),
    ));

    Widget::col(col)
}

fn make_timeline(
    ctx: &mut EventCtx,
    app: &App,
    trip_id: TripID,
    open_trip: &mut OpenTrip,
    details: &mut Details,
    phases: Vec<TripPhase>,
    progress_along_path: Option<f64>,
) -> Widget {
    let map = &app.primary.map;
    let sim = &app.primary.sim;
    let trip = sim.trip_info(trip_id);
    let end_time = phases.last().as_ref().and_then(|p| p.end_time);

    let start_btn = {
        let (id, center, name) = endpoint(&trip.start, map);
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
            GeomBatch::mapspace_svg(ctx.prerender, "system/assets/timeline/start_pos.svg")
                .scale(3.0)
                .color(RewriteColor::Change(Color::WHITE, Color::BLACK))
                .color(RewriteColor::Change(
                    Color::hex("#5B5B5B"),
                    Color::hex("#CC4121"),
                ))
                .centered_on(center),
        );
        details.zoomed.append(
            GeomBatch::mapspace_svg(ctx.prerender, "system/assets/timeline/start_pos.svg")
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
        let (id, center, name) = endpoint(&trip.end, map);
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
            GeomBatch::mapspace_svg(ctx.prerender, "system/assets/timeline/goal_pos.svg")
                .scale(3.0)
                .color(RewriteColor::Change(Color::WHITE, Color::BLACK))
                .color(RewriteColor::Change(
                    Color::hex("#5B5B5B"),
                    Color::hex("#CC4121"),
                ))
                .centered_on(center),
        );
        details.zoomed.append(
            GeomBatch::mapspace_svg(ctx.prerender, "system/assets/timeline/goal_pos.svg")
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

    let total_duration_so_far = end_time.unwrap_or_else(|| sim.time()) - trip.departure;

    let total_width = 0.22 * ctx.canvas.window_width;
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

        // TODO We're manually mixing screenspace_svg, our own geometry, and a centered_on(). Be
        // careful about the scale factor. I'm confused about some of the math here, figured it out
        // by trial and error.
        let scale = ctx.get_scale_factor();
        let phase_width = total_width * percent_duration;
        let rect = Polygon::rectangle(phase_width * scale, 15.0 * scale);
        let mut normal = GeomBatch::from(vec![(color, rect.clone())]);
        if idx == num_phases - 1 {
            if let Some(p) = progress_along_path {
                normal.append(
                    GeomBatch::screenspace_svg(
                        ctx.prerender,
                        "system/assets/timeline/current_pos.svg",
                    )
                    .centered_on(Pt2D::new(p * phase_width * scale, 7.5 * scale)),
                );
            }
        }
        normal.append(
            GeomBatch::screenspace_svg(
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
                    TripPhaseType::Aborted | TripPhaseType::Finished => unreachable!(),
                    TripPhaseType::DelayedStart => "system/assets/timeline/delayed_start.svg",
                    // TODO What icon should represent this?
                    TripPhaseType::Remote => "system/assets/timeline/delayed_start.svg",
                },
            )
            .centered_on(
                // TODO Hardcoded layouting...
                Pt2D::new(0.5 * phase_width * scale, -20.0),
            ),
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
                    format!("examine trip phase {} of {}", idx + 1, trip_id),
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
        // Just fill this in so the indexing doesn't mess up
        if idx == open_trip.cached_routes.len() {
            open_trip.cached_routes.push(None);
        }
    }

    let mut col = vec![
        Widget::custom_row(vec![start_btn, Widget::custom_row(timeline), goal_btn])
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
            {
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
            },
            if let Some(t) = end_time {
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
