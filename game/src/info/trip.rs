use crate::app::App;
use crate::colors;
use crate::helpers::ID;
use crate::info::{make_table, TripDetails};
use crate::render::dashed_lines;
use ezgui::{
    hotkey, Btn, Color, EventCtx, GeomBatch, Key, Line, Plot, PlotOptions, RewriteColor, Series,
    Text, Widget,
};
use geom::{Angle, Distance, Duration, Polygon, Pt2D, Time};
use map_model::{Map, Path, PathStep};
use sim::{TripEnd, TripID, TripPhaseType, TripStart};
use std::collections::HashMap;

pub fn info(
    ctx: &mut EventCtx,
    app: &App,
    id: TripID,
    action_btns: Vec<Widget>,
) -> (Vec<Widget>, Option<TripDetails>) {
    let mut rows = vec![];

    rows.push(Widget::row(vec![
        Line(format!("Trip #{}", id.0)).roboto_bold().draw(ctx),
        // No jump-to-object button; this is probably a finished trip.
        Btn::text_fg("X")
            .build(ctx, "close info", hotkey(Key::Escape))
            .align_right(),
    ]));
    rows.extend(action_btns);

    let (more, details) = trip_details(ctx, app, id, None);
    rows.push(more);

    (rows, Some(details))
}

pub fn trip_details(
    ctx: &mut EventCtx,
    app: &App,
    trip: TripID,
    progress_along_path: Option<f64>,
) -> (Widget, TripDetails) {
    let map = &app.primary.map;
    let phases = app.primary.sim.get_analytics().get_trip_phases(trip, map);
    let (trip_start, trip_end) = app.primary.sim.trip_endpoints(trip);

    let mut unzoomed = GeomBatch::new();
    let mut zoomed = GeomBatch::new();
    let mut markers = HashMap::new();

    let trip_start_time = phases[0].start_time;
    let trip_end_time = phases.last().as_ref().and_then(|p| p.end_time);

    let start_tooltip = match trip_start {
        TripStart::Bldg(b) => {
            let bldg = map.get_b(b);

            markers.insert("jump to start".to_string(), ID::Building(b));

            unzoomed.add_svg(
                ctx.prerender,
                "../data/system/assets/timeline/start_pos.svg",
                bldg.label_center,
                1.0,
                Angle::ZERO,
            );
            zoomed.add_svg(
                ctx.prerender,
                "../data/system/assets/timeline/start_pos.svg",
                bldg.label_center,
                0.5,
                Angle::ZERO,
            );

            let mut txt = Text::from(Line("jump to start"));
            txt.add(Line(bldg.just_address(map)));
            txt.add(Line(phases[0].start_time.ampm_tostring()));
            txt
        }
        TripStart::Border(i) => {
            let i = map.get_i(i);

            markers.insert("jump to start".to_string(), ID::Intersection(i.id));

            unzoomed.add_svg(
                ctx.prerender,
                "../data/system/assets/timeline/start_pos.svg",
                i.polygon.center(),
                1.0,
                Angle::ZERO,
            );
            zoomed.add_svg(
                ctx.prerender,
                "../data/system/assets/timeline/start_pos.svg",
                i.polygon.center(),
                0.5,
                Angle::ZERO,
            );

            let mut txt = Text::from(Line("jump to start"));
            txt.add(Line(i.name(map)));
            txt.add(Line(phases[0].start_time.ampm_tostring()));
            txt
        }
    };
    let start_btn = Btn::svg(
        "../data/system/assets/timeline/start_pos.svg",
        RewriteColor::Change(Color::WHITE, colors::HOVERING),
    )
    .tooltip(start_tooltip)
    .build(ctx, "jump to start", None);

    let goal_tooltip = match trip_end {
        TripEnd::Bldg(b) => {
            let bldg = map.get_b(b);

            markers.insert("jump to goal".to_string(), ID::Building(b));

            unzoomed.add_svg(
                ctx.prerender,
                "../data/system/assets/timeline/goal_pos.svg",
                bldg.label_center,
                1.0,
                Angle::ZERO,
            );
            zoomed.add_svg(
                ctx.prerender,
                "../data/system/assets/timeline/goal_pos.svg",
                bldg.label_center,
                0.5,
                Angle::ZERO,
            );

            let mut txt = Text::from(Line("jump to goal"));
            txt.add(Line(bldg.just_address(map)));
            if let Some(t) = trip_end_time {
                txt.add(Line(t.ampm_tostring()));
            }
            txt
        }
        TripEnd::Border(i) => {
            let i = map.get_i(i);

            markers.insert("jump to goal".to_string(), ID::Intersection(i.id));

            unzoomed.add_svg(
                ctx.prerender,
                "../data/system/assets/timeline/goal_pos.svg",
                i.polygon.center(),
                1.0,
                Angle::ZERO,
            );
            zoomed.add_svg(
                ctx.prerender,
                "../data/system/assets/timeline/goal_pos.svg",
                i.polygon.center(),
                0.5,
                Angle::ZERO,
            );

            let mut txt = Text::from(Line("jump to goal"));
            txt.add(Line(i.name(map)));
            if let Some(t) = trip_end_time {
                txt.add(Line(t.ampm_tostring()));
            }
            txt
        }
        TripEnd::ServeBusRoute(_) => unreachable!(),
    };
    let goal_btn = Btn::svg(
        "../data/system/assets/timeline/goal_pos.svg",
        RewriteColor::Change(Color::WHITE, colors::HOVERING),
    )
    .tooltip(goal_tooltip)
    .build(ctx, "jump to goal", None);

    let total_duration_so_far =
        trip_end_time.unwrap_or_else(|| app.primary.sim.time()) - phases[0].start_time;

    let total_width = 0.3 * ctx.canvas.window_width;
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

        let mut txt = Text::from(Line(&p.phase_type.describe(&app.primary.map)));
        txt.add(Line(format!(
            "- Started at {}",
            p.start_time.ampm_tostring()
        )));
        let duration = if let Some(t2) = p.end_time {
            let d = t2 - p.start_time;
            txt.add(Line(format!("- Ended at {} (duration: {})", t2, d)));
            d
        } else {
            let d = app.primary.sim.time() - p.start_time;
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
            hovered.push(c, p);
        }

        timeline.push(
            Btn::custom(normal, hovered, rect)
                .build(ctx, format!("examine trip phase {}", idx + 1), None)
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
                unzoomed.push(color, trace.make_polygons(Distance::meters(10.0)));
                zoomed.extend(
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

    let mut table = vec![
        ("Trip start".to_string(), trip_start_time.ampm_tostring()),
        ("Duration".to_string(), total_duration_so_far.to_string()),
    ];
    if let Some(t) = trip_end_time {
        table.push(("Trip end".to_string(), t.ampm_tostring()));
    }
    let mut col = vec![Widget::row(timeline).evenly_spaced().margin_above(25)];
    col.extend(make_table(ctx, table));
    col.extend(elevation);
    if let Some(p) = app.primary.sim.trip_to_person(trip) {
        col.push(
            Btn::text_bg1(format!("Trip by Person #{}", p.0))
                .build(ctx, format!("examine Person #{}", p.0), None)
                .margin(5),
        );
    }

    (
        Widget::col(col),
        TripDetails {
            id: trip,
            unzoomed: unzoomed.upload(ctx),
            zoomed: zoomed.upload(ctx),
            markers,
        },
    )
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
    Plot::new_usize(
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
    .bg(colors::PANEL_BG)
}
