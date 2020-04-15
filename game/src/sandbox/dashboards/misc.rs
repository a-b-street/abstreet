use crate::app::App;
use crate::common::Tab;
use crate::game::{msg, State, Transition};
use crate::sandbox::dashboards::DashTab;
use crate::sandbox::SandboxMode;
use ezgui::{
    Btn, Color, Composite, EventCtx, GfxCtx, Line, LinePlot, Outcome, PlotOptions, Series, Widget,
};
use geom::Time;

pub struct ActiveTraffic {
    composite: Composite,
}

impl ActiveTraffic {
    pub fn new(ctx: &mut EventCtx, app: &App) -> Box<dyn State> {
        let mut active_agents = vec![Series {
            label: "After changes".to_string(),
            color: Color::RED,
            pts: app
                .primary
                .sim
                .get_analytics()
                .active_agents(app.primary.sim.time()),
        }];
        if app.has_prebaked().is_some() {
            active_agents.push(Series {
                label: "Before changes".to_string(),
                color: Color::BLUE.alpha(0.5),
                pts: app.prebaked().active_agents(Time::END_OF_DAY),
            });
        }

        Box::new(ActiveTraffic {
            composite: Composite::new(
                Widget::col(vec![
                    DashTab::ActiveTraffic.picker(ctx),
                    LinePlot::new(ctx, "active agents", active_agents, PlotOptions::new()),
                ])
                .bg(app.cs.panel_bg)
                .padding(10),
            )
            .max_size_percent(90, 90)
            .build(ctx),
        })
    }
}

impl State for ActiveTraffic {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => DashTab::TripSummaries.transition(ctx, app, &x),
            None => Transition::Keep,
        }
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        State::grey_out_map(g, app);
        self.composite.draw(g);
    }
}

pub struct BusRoutes {
    composite: Composite,
}

impl BusRoutes {
    pub fn new(ctx: &mut EventCtx, app: &App) -> Box<dyn State> {
        let mut routes: Vec<String> = app
            .primary
            .map
            .get_all_bus_routes()
            .iter()
            .map(|r| r.name.clone())
            .collect();
        // TODO Sort first by length, then lexicographically
        routes.sort();

        let mut col = vec![
            DashTab::BusRoutes.picker(ctx),
            Line("Bus routes").small_heading().draw(ctx),
        ];
        for r in routes {
            col.push(Btn::text_fg(r).build_def(ctx, None).margin(5));
        }

        Box::new(BusRoutes {
            composite: Composite::new(Widget::col(col).bg(app.cs.panel_bg).padding(10))
                .max_size_percent(90, 90)
                .build(ctx),
        })
    }
}

impl State for BusRoutes {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => {
                if let Some(r) = app.primary.map.get_bus_route(&x) {
                    let buses = app.primary.sim.status_of_buses(r.id);
                    if buses.is_empty() {
                        Transition::Push(msg(
                            "No buses running",
                            vec![format!("Sorry, no buses for route {} running", r.name)],
                        ))
                    } else {
                        Transition::PopWithData(Box::new(move |state, app, ctx| {
                            let sandbox = state.downcast_mut::<SandboxMode>().unwrap();
                            let mut actions = sandbox.contextual_actions();
                            sandbox.controls.common.as_mut().unwrap().launch_info_panel(
                                ctx,
                                app,
                                // Arbitrarily use the first one
                                Tab::BusStatus(buses[0].0),
                                &mut actions,
                            );
                        }))
                    }
                } else {
                    DashTab::BusRoutes.transition(ctx, app, &x)
                }
            }
            None => Transition::Keep,
        }
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        State::grey_out_map(g, app);
        self.composite.draw(g);
    }
}
