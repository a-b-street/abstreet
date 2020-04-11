use crate::app::App;
use crate::common::Tab;
use crate::game::{msg, State, Transition};
use crate::managed::{Callback, ManagedGUIState, WrappedComposite};
use crate::sandbox::SandboxMode;
use ezgui::{
    hotkey, Btn, Color, Composite, EventCtx, Key, Line, LinePlot, PlotOptions, Series, Widget,
};
use geom::Time;
use map_model::BusRouteID;

#[derive(PartialEq, Clone, Copy)]
pub enum DashTab {
    TripsSummary,
    ExploreBusRoute,
}

// Oh the dashboards melted, but we still had the radio
pub fn make(ctx: &mut EventCtx, app: &App, tab: DashTab) -> Box<dyn State> {
    let tab_data = vec![
        (DashTab::TripsSummary, "Trips summary"),
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
        DashTab::TripsSummary => (trips_summary(ctx, app), Vec::new()),
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

fn trips_summary(ctx: &EventCtx, app: &App) -> Widget {
    Widget::col(vec![
        Line("Active agents").small_heading().draw(ctx),
        LinePlot::new(
            ctx,
            "active agents",
            if app.has_prebaked().is_some() {
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
                ]
            } else {
                vec![Series {
                    label: "Current simulation".to_string(),
                    color: Color::RED,
                    pts: app
                        .primary
                        .sim
                        .get_analytics()
                        .active_agents(app.primary.sim.time()),
                }]
            },
            PlotOptions::new(),
        ),
    ])
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
