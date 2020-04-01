use crate::app::App;
use crate::colors;
use crate::helpers::ID;
use crate::info::{make_table, Details};
use crate::render::dashed_lines;
use ezgui::{
    Btn, Color, EventCtx, GeomBatch, Line, Plot, PlotOptions, RewriteColor, Series, Text, TextExt,
    Widget,
};
use geom::{Angle, Distance, Duration, Polygon, Pt2D, Time};
use map_model::{Map, Path, PathStep};
use sim::{AgentID, TripEndpoint, TripID, TripPhase, TripPhaseType};

pub fn details(ctx: &mut EventCtx, app: &App, trip: TripID, details: &mut Details) -> Widget {
    let map = &app.primary.map;
    let sim = &app.primary.sim;
    let phases = sim.get_analytics().get_trip_phases(trip, map);
    let (start_time, trip_start, trip_end, trip_mode) = sim.trip_info(trip);
    let end_time = phases.last().as_ref().and_then(|p| p.end_time);

    if phases.is_empty() {
        // The trip hasn't started

        // TODO Warp buttons. make_table is showing its age.
        let (_, _, name1) = endpoint(&trip_start, map);
        let (_, _, name2) = endpoint(&trip_end, map);
        return Widget::col(make_table(
            ctx,
            vec![
                ("Type", trip_mode.to_string()),
                ("Departure", start_time.ampm_tostring()),
                ("From", name1),
                ("To", name2),
            ],
        ));
    }

    let mut col = Vec::new();
    let total_duration_so_far = end_time.unwrap_or_else(|| sim.time()) - phases[0].start_time;
    let mut kv = vec![
        ("Type", trip_mode.to_string()),
        ("Duration", total_duration_so_far.to_string()),
    ];
    let progress_along_path = if let Some(a) = sim.trip_to_agent(trip).ok() {
        let (more_kv, extra) = match a {
            AgentID::Car(c) => (sim.car_properties(c, map), Vec::new()),
            AgentID::Pedestrian(p) => sim.ped_properties(p, map),
        };
        for (k, v) in &more_kv {
            kv.push((k, v.to_string()));
        }
        col.extend(make_table(ctx, kv));

        if !extra.is_empty() {
            let mut txt = Text::from(Line(""));
            for line in extra {
                txt.add(Line(line));
            }
            col.push(txt.draw(ctx));
        }

        sim.progress_along_path(a)
    } else {
        col.extend(make_table(ctx, kv));
        None
    };

    col.push(make_timeline(
        ctx,
        app,
        trip,
        details,
        phases,
        progress_along_path,
    ));

    Widget::col(col)
}

fn make_timeline(
    ctx: &mut EventCtx,
    app: &App,
    trip: TripID,
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
        details.unzoomed.add_svg(
            ctx.prerender,
            "../data/system/assets/timeline/start_pos.svg",
            center,
            1.0,
            Angle::ZERO,
        );
        details.zoomed.add_svg(
            ctx.prerender,
            "../data/system/assets/timeline/start_pos.svg",
            center,
            0.5,
            Angle::ZERO,
        );
        let mut txt = Text::from(Line(name));
        txt.add(Line(start_time.ampm_tostring()));
        Btn::svg(
            "../data/system/assets/timeline/start_pos.svg",
            RewriteColor::Change(Color::WHITE, colors::HOVERING),
        )
        .tooltip(txt)
        .build(ctx, format!("jump to start of {}", trip), None)
    };

    let goal_btn = {
        let (id, center, name) = endpoint(&trip_end, map);
        details
            .warpers
            .insert(format!("jump to goal of {}", trip), id);
        details.unzoomed.add_svg(
            ctx.prerender,
            "../data/system/assets/timeline/goal_pos.svg",
            center,
            1.0,
            Angle::ZERO,
        );
        details.zoomed.add_svg(
            ctx.prerender,
            "../data/system/assets/timeline/goal_pos.svg",
            center,
            0.5,
            Angle::ZERO,
        );
        let mut txt = Text::from(Line(name));
        if let Some(t) = end_time {
            txt.add(Line(t.ampm_tostring()));
        }
        Btn::svg(
            "../data/system/assets/timeline/goal_pos.svg",
            RewriteColor::Change(Color::WHITE, colors::HOVERING),
        )
        .tooltip(txt)
        .build(ctx, format!("jump to goal of {}", trip), None)
    };

    let total_duration_so_far = end_time.unwrap_or_else(|| sim.time()) - phases[0].start_time;

    let total_width = 0.22 * ctx.canvas.window_width;
    let mut timeline = Vec::new();
    let num_phases = phases.len();
    let mut elevation = Vec::new();
    for (idx, p) in phases.into_iter().enumerate() {
        let color = match p.phase_type {
            TripPhaseType::Driving => Color::hex("#D63220"),
            TripPhaseType::Walking => Color::hex("#DF8C3D"),
            TripPhaseType::Biking => app.cs.get("bike lane"),
            TripPhaseType::Parking => Color::hex("#4E30A6"),
            TripPhaseType::WaitingForBus(_) => app.cs.get("bus stop marking"),
            TripPhaseType::RidingBus(_) => app.cs.get("bus lane"),
            TripPhaseType::Aborted | TripPhaseType::Finished => unreachable!(),
        }
        .alpha(0.7);

        let mut txt = Text::from(Line(&p.phase_type.describe(map)));
        txt.add(Line(format!(
            "- Started at {}",
            p.start_time.ampm_tostring()
        )));
        let duration = if let Some(t2) = p.end_time {
            let d = t2 - p.start_time;
            txt.add(Line(format!("- Ended at {} (duration: {})", t2, d)));
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
                    Pt2D::new(p * phase_width, 7.5),
                    1.0,
                    Angle::ZERO,
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
                TripPhaseType::WaitingForBus(_) => {
                    "../data/system/assets/timeline/waiting_for_bus.svg"
                }
                TripPhaseType::RidingBus(_) => "../data/system/assets/timeline/riding_bus.svg",
                TripPhaseType::Aborted | TripPhaseType::Finished => unreachable!(),
            },
            // TODO Hardcoded layouting...
            Pt2D::new(0.5 * phase_width, -20.0),
            1.0,
            Angle::ZERO,
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

        // TODO Could really cache this between live updates
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

            if let Some(trace) = path.trace(map, dist, None) {
                details
                    .unzoomed
                    .push(color, trace.make_polygons(Distance::meters(10.0)));
                details.zoomed.extend(
                    color,
                    dashed_lines(
                        &trace,
                        Distance::meters(0.75),
                        Distance::meters(1.0),
                        Distance::meters(0.4),
                    ),
                );
            }
        }
    }

    timeline.insert(0, start_btn.margin(5));
    timeline.push(goal_btn.margin(5));
    let mut col = vec![
        Widget::row(timeline).evenly_spaced().margin_above(25),
        if let Some(t) = end_time {
            Widget::row(vec![
                start_time.ampm_tostring().draw_text(ctx),
                t.ampm_tostring().draw_text(ctx).align_right(),
            ])
        } else {
            start_time.ampm_tostring().draw_text(ctx)
        },
    ];
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
    Plot::new(
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
        PlotOptions::new(),
    )
}

// (ID, center, name)
fn endpoint(endpt: &TripEndpoint, map: &Map) -> (ID, Pt2D, String) {
    match endpt {
        TripEndpoint::Bldg(b) => {
            let bldg = map.get_b(*b);
            (ID::Building(*b), bldg.label_center, bldg.just_address(map))
        }
        TripEndpoint::Border(i) => {
            let i = map.get_i(*i);
            (ID::Intersection(i.id), i.polygon.center(), i.name(map))
        }
    }
}
