use crate::app::App;
use crate::colors;
use crate::helpers::ID;
use crate::info::{make_table, make_tabs, person, InfoTab, TripDetails};
use crate::render::dashed_lines;
use ezgui::{
    hotkey, Btn, Color, EventCtx, GeomBatch, Key, Line, Plot, PlotOptions, RewriteColor, Series,
    Text, Widget,
};
use geom::{Angle, Distance, Duration, Polygon, Pt2D, Time};
use map_model::{Map, Path, PathStep};
use sim::{PersonID, TripEnd, TripID, TripPhaseType, TripStart};
use std::collections::HashMap;

#[derive(Clone, PartialEq)]
pub enum Tab {
    Person(PersonID),
}

pub fn inactive_info(
    ctx: &mut EventCtx,
    app: &App,
    id: TripID,
    tab: InfoTab,
    action_btns: Vec<Widget>,
    hyperlinks: &mut HashMap<String, (ID, InfoTab)>,
) -> (Vec<Widget>, Option<TripDetails>) {
    let mut rows = vec![];

    rows.push(Widget::row(vec![
        Line(format!("Trip #{}", id.0)).roboto_bold().draw(ctx),
        Btn::text_fg("X")
            .build(ctx, "close info", hotkey(Key::Escape))
            .align_right(),
    ]));

    rows.push(make_tabs(
        ctx,
        hyperlinks,
        ID::Trip(id),
        tab.clone(),
        vec![
            ("Info", InfoTab::Nil),
            (
                "Schedule",
                InfoTab::Trip(Tab::Person(app.primary.sim.trip_to_person(id))),
            ),
        ],
    ));

    let mut details: Option<TripDetails> = None;

    match tab {
        InfoTab::Nil => {
            rows.extend(action_btns);

            let (more, trip_details) = trip_details(ctx, app, id, None);
            rows.push(more);
            details = Some(trip_details);
        }
        InfoTab::Trip(Tab::Person(p)) => {
            rows.extend(person::info(ctx, app, p, None, Vec::new(), hyperlinks));
        }
        _ => unreachable!(),
    }

    (rows, details)
}

pub fn trip_details(
    ctx: &mut EventCtx,
    app: &App,
    trip: TripID,
    progress_along_path: Option<f64>,
) -> (Widget, TripDetails) {
    let map = &app.primary.map;
    let phases = app.primary.sim.get_analytics().get_trip_phases(trip, map);
    let (start_time, trip_start, trip_end, trip_mode) = app.primary.sim.trip_info(trip);

    let mut unzoomed = GeomBatch::new();
    let mut zoomed = GeomBatch::new();
    let mut markers = HashMap::new();

    if phases.is_empty() {
        // The trip hasn't started
        let kv = vec![
            ("Trip start", start_time.ampm_tostring()),
            ("Type", trip_mode.to_string()),
            // TODO If we're looking at a building, then "here"...
            // TODO Refactor... TripStart.name(map)?
            // TODO Buttons
            (
                "From",
                match trip_start {
                    TripStart::Bldg(b) => map.get_b(b).just_address(map),
                    TripStart::Border(i) => map.get_i(i).name(map),
                },
            ),
            (
                "To",
                match trip_end {
                    TripEnd::Bldg(b) => map.get_b(b).just_address(map),
                    TripEnd::Border(i) => map.get_i(i).name(map),
                },
            ),
        ];
        return (
            Widget::col(make_table(ctx, kv)),
            TripDetails {
                id: trip,
                unzoomed: unzoomed.upload(ctx),
                zoomed: zoomed.upload(ctx),
                markers,
            },
        );
    }

    let start_btn = {
        let (id, center, name) = match trip_start {
            TripStart::Bldg(b) => {
                let bldg = map.get_b(b);
                (ID::Building(b), bldg.label_center, bldg.just_address(map))
            }
            TripStart::Border(i) => {
                let i = map.get_i(i);
                (ID::Intersection(i.id), i.polygon.center(), i.name(map))
            }
        };
        markers.insert("jump to start".to_string(), id);
        unzoomed.add_svg(
            ctx.prerender,
            "../data/system/assets/timeline/start_pos.svg",
            center,
            1.0,
            Angle::ZERO,
        );
        zoomed.add_svg(
            ctx.prerender,
            "../data/system/assets/timeline/start_pos.svg",
            center,
            0.5,
            Angle::ZERO,
        );
        let mut txt = Text::from(Line("jump to start"));
        txt.add(Line(name));
        txt.add(Line(start_time.ampm_tostring()));
        Btn::svg(
            "../data/system/assets/timeline/start_pos.svg",
            RewriteColor::Change(Color::WHITE, colors::HOVERING),
        )
        .tooltip(txt)
        .build(ctx, "jump to start", None)
    };

    let end_time = phases.last().as_ref().and_then(|p| p.end_time);

    let goal_btn = {
        let (id, center, name) = match trip_end {
            TripEnd::Bldg(b) => {
                let bldg = map.get_b(b);
                (ID::Building(b), bldg.label_center, bldg.just_address(map))
            }
            TripEnd::Border(i) => {
                let i = map.get_i(i);
                (ID::Intersection(i.id), i.polygon.center(), i.name(map))
            }
        };
        markers.insert("jump to goal".to_string(), id);
        unzoomed.add_svg(
            ctx.prerender,
            "../data/system/assets/timeline/goal_pos.svg",
            center,
            1.0,
            Angle::ZERO,
        );
        zoomed.add_svg(
            ctx.prerender,
            "../data/system/assets/timeline/goal_pos.svg",
            center,
            0.5,
            Angle::ZERO,
        );
        let mut txt = Text::from(Line("jump to goal"));
        txt.add(Line(name));
        if let Some(t) = end_time {
            txt.add(Line(t.ampm_tostring()));
        }
        Btn::svg(
            "../data/system/assets/timeline/goal_pos.svg",
            RewriteColor::Change(Color::WHITE, colors::HOVERING),
        )
        .tooltip(txt)
        .build(ctx, "jump to goal", None)
    };

    let total_duration_so_far =
        end_time.unwrap_or_else(|| app.primary.sim.time()) - phases[0].start_time;

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
                .tooltip(txt)
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

    let mut kv = vec![
        ("Trip start", start_time.ampm_tostring()),
        ("Duration", total_duration_so_far.to_string()),
    ];
    if let Some(t) = end_time {
        kv.push(("Trip end", t.ampm_tostring()));
    }
    let mut col = vec![Widget::row(timeline).evenly_spaced().margin_above(25)];
    col.extend(make_table(ctx, kv));
    col.extend(elevation);

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
