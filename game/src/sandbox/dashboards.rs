use crate::app::App;
use crate::common::Tab;
use crate::game::{msg, State, Transition};
use crate::helpers::{cmp_count_fewer, cmp_count_more, cmp_duration_shorter};
use crate::managed::{Callback, ManagedGUIState, WrappedComposite};
use crate::sandbox::SandboxMode;
use abstutil::prettyprint_usize;
use abstutil::Counter;
use ezgui::{
    hotkey, Btn, Color, Composite, EventCtx, Key, Line, Plot, PlotOptions, Series, Text, TextExt,
    Widget,
};
use geom::{Statistic, Time};
use map_model::BusRouteID;
use sim::TripMode;
use std::collections::BTreeMap;

#[derive(PartialEq, Clone, Copy)]
pub enum DashTab {
    TripsSummary,
    ParkingOverhead,
    ExploreBusRoute,
}

// Oh the dashboards melted, but we still had the radio
pub fn make(ctx: &mut EventCtx, app: &App, tab: DashTab) -> Box<dyn State> {
    let tab_data = vec![
        (DashTab::TripsSummary, "Trips summary"),
        (DashTab::ParkingOverhead, "Parking overhead analysis"),
        (DashTab::ExploreBusRoute, "Explore a bus route"),
    ];

    let tabs = tab_data
        .iter()
        .map(|(t, label)| {
            if *t == tab {
                Btn::text_bg2(*label).inactive(ctx)
            } else {
                Btn::text_bg2(*label).build_def(ctx, None)
            }
        })
        .collect::<Vec<_>>();

    let (content, cbs) = match tab {
        DashTab::TripsSummary => (trips_summary_prebaked(ctx, app), Vec::new()),
        DashTab::ParkingOverhead => (parking_overhead(ctx, app), Vec::new()),
        DashTab::ExploreBusRoute => pick_bus_route(ctx, app),
    };

    let mut c = WrappedComposite::new(
        Composite::new(Widget::col(vec![
            Btn::svg_def("../data/system/assets/pregame/back.svg")
                .build(ctx, "back", hotkey(Key::Escape))
                .align_left(),
            Widget::row(tabs).bg(app.cs.panel_bg),
            content.bg(app.cs.panel_bg),
        ]))
        // TODO Want to use exact, but then scrolling breaks. exact_size_percent will fix the
        // jumpiness though.
        .max_size_percent(90, 80)
        .build(ctx),
    )
    .cb("back", Box::new(|_, _| Some(Transition::Pop)));
    for (t, label) in tab_data {
        if t != tab {
            c = c.cb(
                label,
                Box::new(move |ctx, app| Some(Transition::Replace(make(ctx, app, t)))),
            );
        }
    }
    for (name, cb) in cbs {
        c = c.cb(&name, cb);
    }

    ManagedGUIState::fullscreen(c)
}

// TODO Overhaul typography.
fn trips_summary_prebaked(ctx: &EventCtx, app: &App) -> Widget {
    if app.has_prebaked().is_none() {
        return trips_summary_not_prebaked(ctx, app);
    }

    let (now_all, now_aborted, now_per_mode) = app
        .primary
        .sim
        .get_analytics()
        .trip_times(app.primary.sim.time());
    let (baseline_all, baseline_aborted, baseline_per_mode) =
        app.prebaked().trip_times(app.primary.sim.time());

    // TODO Include unfinished count
    let mut txt = Text::new();
    txt.add(Line(format!(
        "Trips as of {}",
        app.primary.sim.time().ampm_tostring()
    )));
    txt.add_appended(vec![
        Line(format!(
            "{} aborted trips (",
            prettyprint_usize(now_aborted)
        )),
        cmp_count_fewer(now_aborted, baseline_aborted),
        Line(")"),
    ]);
    // TODO Refactor
    txt.add_appended(vec![
        Line(format!(
            "{} total trips (",
            prettyprint_usize(now_all.count())
        )),
        cmp_count_more(now_all.count(), baseline_all.count()),
        Line(")"),
    ]);
    if now_all.count() > 0 && baseline_all.count() > 0 {
        for stat in Statistic::all() {
            // TODO Ideally we could indent
            txt.add(Line(format!("{}: {} (", stat, now_all.select(stat))));
            txt.append_all(cmp_duration_shorter(
                now_all.select(stat),
                baseline_all.select(stat),
            ));
            txt.append(Line(")"));
        }
    }

    for mode in TripMode::all() {
        let a = &now_per_mode[&mode];
        let b = &baseline_per_mode[&mode];
        txt.add_appended(vec![
            Line(format!(
                "{} trips {} (",
                prettyprint_usize(a.count()),
                mode.ongoing_verb()
            )),
            cmp_count_more(a.count(), b.count()),
            Line(")"),
        ]);
        if a.count() > 0 && b.count() > 0 {
            for stat in Statistic::all() {
                txt.add(Line(format!("{}: {} (", stat, a.select(stat))));
                txt.append_all(cmp_duration_shorter(a.select(stat), b.select(stat)));
                txt.append(Line(")"));
            }
        }
    }

    Widget::col(vec![
        txt.draw(ctx),
        finished_trips_plot(ctx, app).bg(app.cs.section_bg),
        Line("Active agents").small_heading().draw(ctx),
        Plot::new(
            ctx,
            "active agents",
            vec![
                Series {
                    label: "Baseline".to_string(),
                    color: Color::BLUE.alpha(0.5),
                    pts: app.prebaked().active_agents(Time::END_OF_DAY),
                },
                Series {
                    label: "Current simulation".to_string(),
                    color: Color::RED,
                    pts: app
                        .primary
                        .sim
                        .get_analytics()
                        .active_agents(app.primary.sim.time()),
                },
            ],
            PlotOptions::new(),
        ),
    ])
}

fn trips_summary_not_prebaked(ctx: &EventCtx, app: &App) -> Widget {
    let (all, aborted, per_mode) = app
        .primary
        .sim
        .get_analytics()
        .trip_times(app.primary.sim.time());

    // TODO Include unfinished count
    let mut txt = Text::new();
    txt.add(Line(format!(
        "Trips as of {}",
        app.primary.sim.time().ampm_tostring()
    )));
    txt.add(Line(format!(
        "{} aborted trips",
        prettyprint_usize(aborted)
    )));
    txt.add(Line(format!(
        "{} total trips",
        prettyprint_usize(all.count())
    )));
    if all.count() > 0 {
        for stat in Statistic::all() {
            txt.add(Line(format!("{}: {}", stat, all.select(stat))));
        }
    }

    for mode in TripMode::all() {
        let a = &per_mode[&mode];
        txt.add(Line(format!(
            "{} trips {}",
            prettyprint_usize(a.count()),
            mode.ongoing_verb()
        )));
        if a.count() > 0 {
            for stat in Statistic::all() {
                txt.add(Line(format!("{}: {}", stat, a.select(stat))));
            }
        }
    }

    Widget::col(vec![
        txt.draw(ctx),
        finished_trips_plot(ctx, app).bg(app.cs.section_bg),
        Line("Active agents").small_heading().draw(ctx),
        Plot::new(
            ctx,
            "active agents",
            vec![Series {
                label: "Active agents".to_string(),
                color: Color::RED,
                pts: app
                    .primary
                    .sim
                    .get_analytics()
                    .active_agents(app.primary.sim.time()),
            }],
            PlotOptions::new(),
        ),
    ])
}

fn finished_trips_plot(ctx: &EventCtx, app: &App) -> Widget {
    let mut lines: Vec<(String, Color, Option<TripMode>)> = TripMode::all()
        .into_iter()
        .map(|m| {
            (
                m.ongoing_verb().to_string(),
                color_for_mode(m, app),
                Some(m),
            )
        })
        .collect();
    lines.push(("aborted".to_string(), Color::PURPLE.alpha(0.5), None));

    // What times do we use for interpolation?
    let num_x_pts = 100;
    let mut times = Vec::new();
    for i in 0..num_x_pts {
        let percent_x = (i as f64) / ((num_x_pts - 1) as f64);
        let t = app.primary.sim.time().percent_of(percent_x);
        times.push(t);
    }

    // Gather the data
    let mut counts = Counter::new();
    let mut pts_per_mode: BTreeMap<Option<TripMode>, Vec<(Time, usize)>> =
        lines.iter().map(|(_, _, m)| (*m, Vec::new())).collect();
    for (t, _, m, _) in &app.primary.sim.get_analytics().finished_trips {
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
            .push((app.primary.sim.time(), counts.get(*mode)));
    }

    let plot = Plot::new(
        ctx,
        "finished trips",
        lines
            .into_iter()
            .map(|(label, color, m)| Series {
                label,
                color,
                pts: pts_per_mode.remove(&m).unwrap(),
            })
            .collect(),
        PlotOptions::new(),
    );
    Widget::col(vec!["finished trips".draw_text(ctx), plot.margin(10)])
}

fn parking_overhead(ctx: &EventCtx, app: &App) -> Widget {
    let mut txt = Text::new();
    for line in app.primary.sim.get_analytics().analyze_parking_phases() {
        txt.add_wrapped(line, 0.9 * ctx.canvas.window_width);
    }
    txt.draw(ctx)
}

fn pick_bus_route(ctx: &EventCtx, app: &App) -> (Widget, Vec<(String, Callback)>) {
    let mut buttons = Vec::new();
    let mut cbs: Vec<(String, Callback)> = Vec::new();

    let mut routes: Vec<(&String, BusRouteID)> = app
        .primary
        .map
        .get_all_bus_routes()
        .iter()
        .map(|r| (&r.name, r.id))
        .collect();
    // TODO Sort first by length, then lexicographically
    routes.sort_by_key(|(name, _)| name.to_string());

    for (name, id) in routes {
        buttons.push(Btn::text_fg(name).build_def(ctx, None));
        let route_name = name.to_string();
        cbs.push((
            name.to_string(),
            Box::new(move |_, app| {
                let buses = app.primary.sim.status_of_buses(id);
                if buses.is_empty() {
                    Some(Transition::Push(msg(
                        "No buses running",
                        vec![format!("Sorry, no buses for route {} running", route_name)],
                    )))
                } else {
                    Some(Transition::PopWithData(Box::new(move |state, app, ctx| {
                        let sandbox = state.downcast_mut::<SandboxMode>().unwrap();
                        let mut actions = sandbox.contextual_actions();
                        sandbox.controls.common.as_mut().unwrap().launch_info_panel(
                            ctx,
                            app,
                            // Arbitrarily use the first one
                            Tab::BusStatus(buses[0].0),
                            &mut actions,
                        );
                    })))
                }
            }),
        ));
    }

    (Widget::row(buttons).flex_wrap(ctx, 80), cbs)
}

// TODO Refactor
fn color_for_mode(m: TripMode, app: &App) -> Color {
    match m {
        TripMode::Walk => app.cs.unzoomed_pedestrian,
        TripMode::Bike => app.cs.unzoomed_bike,
        TripMode::Transit => app.cs.unzoomed_bus,
        TripMode::Drive => app.cs.unzoomed_car,
    }
}
