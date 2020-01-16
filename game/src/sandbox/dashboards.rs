use crate::common::TripExplorer;
use crate::game::{State, Transition};
use crate::managed::{Callback, Composite, ManagedGUIState};
use crate::sandbox::bus_explorer;
use crate::sandbox::gameplay::{cmp_count_fewer, cmp_count_more, cmp_duration_shorter};
use crate::ui::UI;
use abstutil::prettyprint_usize;
use abstutil::Counter;
use ezgui::{hotkey, Color, EventCtx, Histogram, Key, Line, ManagedWidget, Plot, Series, Text};
use geom::{Duration, Statistic, Time};
use map_model::BusRouteID;
use sim::{TripID, TripMode};
use std::collections::BTreeMap;

#[derive(PartialEq, Clone, Copy)]
pub enum Tab {
    FinishedTripsSummary,
    IndividualFinishedTrips(Option<TripMode>),
    ParkingOverhead,
    ExploreBusRoute,
}

// Oh the dashboards melted, but we still had the radio
pub fn make(ctx: &mut EventCtx, ui: &UI, tab: Tab) -> Box<dyn State> {
    let tab_data = vec![
        (Tab::FinishedTripsSummary, "Finished trips summary"),
        (
            Tab::IndividualFinishedTrips(None),
            "Deep-dive into individual finished trips",
        ),
        (Tab::ParkingOverhead, "Parking overhead analysis"),
        (Tab::ExploreBusRoute, "Explore a bus route"),
    ];

    let mut tabs = tab_data
        .iter()
        .map(|(t, label)| {
            if *t == tab {
                ManagedWidget::draw_text(ctx, Text::from(Line(*label))).margin(5)
            } else {
                Composite::text_button(ctx, label, None).margin(5)
            }
        })
        .collect::<Vec<_>>();
    tabs.push(Composite::text_button(ctx, "BACK", hotkey(Key::Escape)).margin(5));

    let (content, cbs) = match tab {
        Tab::FinishedTripsSummary => (finished_trips_summary(ctx, ui), Vec::new()),
        Tab::IndividualFinishedTrips(None) => pick_finished_trips_mode(ctx),
        Tab::IndividualFinishedTrips(Some(m)) => pick_finished_trips(m, ctx, ui),
        Tab::ParkingOverhead => (parking_overhead(ctx, ui), Vec::new()),
        Tab::ExploreBusRoute => pick_bus_route(ctx, ui),
    };

    let mut c = Composite::new(
        ezgui::Composite::new(ManagedWidget::col(vec![
            ManagedWidget::row(tabs)
                .evenly_spaced()
                .bg(Color::grey(0.6))
                .padding(10),
            content,
        ]))
        // Leave room for OSD
        .max_size_percent(100, 85)
        .build(ctx),
    )
    .cb("BACK", Box::new(|_, _| Some(Transition::Pop)));
    for (t, label) in tab_data {
        // TODO Not quite... all the IndividualFinishedTrips variants need to act the same
        if t != tab {
            c = c.cb(
                label,
                Box::new(move |ctx, ui| Some(Transition::Replace(make(ctx, ui, t)))),
            );
        }
    }
    for (name, cb) in cbs {
        c = c.cb(&name, cb);
    }

    ManagedGUIState::fullscreen(c)
}

fn finished_trips_summary(ctx: &EventCtx, ui: &UI) -> ManagedWidget {
    let (now_all, now_aborted, now_per_mode) = ui
        .primary
        .sim
        .get_analytics()
        .all_finished_trips(ui.primary.sim.time());
    let (baseline_all, baseline_aborted, baseline_per_mode) =
        ui.prebaked().all_finished_trips(ui.primary.sim.time());

    // TODO Include unfinished count
    let mut txt = Text::new();
    txt.add_appended(vec![
        Line("Finished trips as of "),
        Line(ui.primary.sim.time().ampm_tostring()).fg(Color::CYAN),
    ]);
    txt.add_appended(vec![
        Line(format!(
            "  {} aborted trips (",
            prettyprint_usize(now_aborted)
        )),
        cmp_count_fewer(now_aborted, baseline_aborted),
        Line(")"),
    ]);
    // TODO Refactor
    txt.add_appended(vec![
        Line(format!(
            "{} total finished trips (",
            prettyprint_usize(now_all.count())
        )),
        cmp_count_more(now_all.count(), baseline_all.count()),
        Line(")"),
    ]);
    if now_all.count() > 0 && baseline_all.count() > 0 {
        for stat in Statistic::all() {
            txt.add(Line(format!("  {}: {} ", stat, now_all.select(stat))));
            txt.append_all(cmp_duration_shorter(
                now_all.select(stat),
                baseline_all.select(stat),
            ));
        }
    }

    for mode in TripMode::all() {
        let a = &now_per_mode[&mode];
        let b = &baseline_per_mode[&mode];
        txt.add_appended(vec![
            Line(format!("{} {} trips (", prettyprint_usize(a.count()), mode)),
            cmp_count_more(a.count(), b.count()),
            Line(")"),
        ]);
        if a.count() > 0 && b.count() > 0 {
            for stat in Statistic::all() {
                txt.add(Line(format!("  {}: {} ", stat, a.select(stat))));
                txt.append_all(cmp_duration_shorter(a.select(stat), b.select(stat)));
            }
        }
    }

    // TODO The x-axes for the plot and histogram get stretched to the full screen. Don't do that!
    ManagedWidget::col(vec![
        ManagedWidget::draw_text(ctx, txt),
        finished_trips_plot(ctx, ui).bg(Color::grey(0.3)),
        ManagedWidget::draw_text(
            ctx,
            Text::from(Line(
                "Are finished trips faster or slower than the baseline?",
            )),
        ),
        Histogram::new(
            ui.primary
                .sim
                .get_analytics()
                .finished_trip_deltas(ui.primary.sim.time(), ui.prebaked()),
            ctx,
        )
        .bg(Color::grey(0.3)),
    ])
}

fn finished_trips_plot(ctx: &EventCtx, ui: &UI) -> ManagedWidget {
    let mut lines: Vec<(String, Color, Option<TripMode>)> = TripMode::all()
        .into_iter()
        .map(|m| (m.to_string(), color_for_mode(m, ui), Some(m)))
        .collect();
    lines.push(("aborted".to_string(), Color::PURPLE.alpha(0.5), None));

    // What times do we use for interpolation?
    let num_x_pts = 100;
    let mut times = Vec::new();
    for i in 0..num_x_pts {
        let percent_x = (i as f64) / ((num_x_pts - 1) as f64);
        let t = ui.primary.sim.time().percent_of(percent_x);
        times.push(t);
    }

    // Gather the data
    let mut counts = Counter::new();
    let mut pts_per_mode: BTreeMap<Option<TripMode>, Vec<(Time, usize)>> =
        lines.iter().map(|(_, _, m)| (*m, Vec::new())).collect();
    for (t, _, m, _) in &ui.primary.sim.get_analytics().finished_trips {
        counts.inc(*m);
        if *t > times[0] {
            times.remove(0);
            for (_, _, mode) in &lines {
                pts_per_mode
                    .get_mut(mode)
                    .unwrap()
                    .push((*t, counts.get(*mode)));
            }
        }
    }
    // Don't forget the last batch
    for (_, _, mode) in &lines {
        pts_per_mode
            .get_mut(mode)
            .unwrap()
            .push((ui.primary.sim.time(), counts.get(*mode)));
    }

    let plot = Plot::new_usize(
        lines
            .into_iter()
            .map(|(label, color, m)| Series {
                label,
                color,
                pts: pts_per_mode.remove(&m).unwrap(),
            })
            .collect(),
        ctx,
    );
    ManagedWidget::col(vec![
        ManagedWidget::draw_text(ctx, Text::from(Line("finished trips"))),
        plot.margin(10),
    ])
}

fn pick_finished_trips_mode(ctx: &EventCtx) -> (ManagedWidget, Vec<(String, Callback)>) {
    let mut buttons = Vec::new();
    let mut cbs: Vec<(String, Callback)> = Vec::new();

    for mode in TripMode::all() {
        buttons.push(Composite::text_button(ctx, &mode.to_string(), None));
        cbs.push((
            mode.to_string(),
            Box::new(move |ctx, ui| {
                Some(Transition::Replace(make(
                    ctx,
                    ui,
                    Tab::IndividualFinishedTrips(Some(mode)),
                )))
            }),
        ));
    }

    (ManagedWidget::row(buttons).flex_wrap(ctx, 80), cbs)
}

fn pick_finished_trips(
    mode: TripMode,
    ctx: &EventCtx,
    ui: &UI,
) -> (ManagedWidget, Vec<(String, Callback)>) {
    let mut buttons = Vec::new();
    let mut cbs: Vec<(String, Callback)> = Vec::new();

    let mut filtered: Vec<&(Time, TripID, Option<TripMode>, Duration)> = ui
        .primary
        .sim
        .get_analytics()
        .finished_trips
        .iter()
        .filter(|(_, _, m, _)| *m == Some(mode))
        .collect();
    filtered.sort_by_key(|(_, _, _, dt)| *dt);
    filtered.reverse();
    for (_, id, _, dt) in filtered {
        let label = format!("{} taking {}", id, dt);
        buttons.push(Composite::text_button(ctx, &label, None));
        let trip = *id;
        cbs.push((
            label,
            Box::new(move |ctx, ui| {
                Some(Transition::Push(Box::new(TripExplorer::new(trip, ctx, ui))))
            }),
        ));
    }

    // TODO Indicate the current mode
    let (mode_picker, more_cbs) = pick_finished_trips_mode(ctx);
    cbs.extend(more_cbs);

    (
        ManagedWidget::col(vec![
            mode_picker,
            ManagedWidget::row(buttons).flex_wrap(ctx, 80),
        ]),
        cbs,
    )
}

fn parking_overhead(ctx: &EventCtx, ui: &UI) -> ManagedWidget {
    let mut txt = Text::new();
    for line in ui.primary.sim.get_analytics().analyze_parking_phases() {
        txt.add_wrapped(line, 0.9 * ctx.canvas.window_width);
    }
    ManagedWidget::draw_text(ctx, txt)
}

fn pick_bus_route(ctx: &EventCtx, ui: &UI) -> (ManagedWidget, Vec<(String, Callback)>) {
    let mut buttons = Vec::new();
    let mut cbs: Vec<(String, Callback)> = Vec::new();

    let mut routes: Vec<(&String, BusRouteID)> = ui
        .primary
        .map
        .get_all_bus_routes()
        .iter()
        .map(|r| (&r.name, r.id))
        .collect();
    // TODO Sort first by length, then lexicographically
    routes.sort_by_key(|(name, _)| name.to_string());

    for (name, id) in routes {
        buttons.push(Composite::text_button(ctx, name, None));
        cbs.push((
            name.to_string(),
            Box::new(move |_, _| Some(Transition::Push(bus_explorer::make_route_picker(vec![id])))),
        ));
    }

    (ManagedWidget::row(buttons).flex_wrap(ctx, 80), cbs)
}

// TODO Refactor
fn color_for_mode(m: TripMode, ui: &UI) -> Color {
    match m {
        TripMode::Walk => ui.cs.get("unzoomed pedestrian"),
        TripMode::Bike => ui.cs.get("unzoomed bike"),
        TripMode::Transit => ui.cs.get("unzoomed bus"),
        TripMode::Drive => ui.cs.get("unzoomed car"),
    }
}
