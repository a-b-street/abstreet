use crate::app::App;
use crate::common::Tab;
use crate::game::{DrawBaselayer, State, Transition};
use crate::sandbox::dashboards::DashTab;
use crate::sandbox::SandboxMode;
use abstutil::{prettyprint_usize, Counter};
use ezgui::{
    Autocomplete, Btn, Composite, EventCtx, GfxCtx, Line, LinePlot, Outcome, PlotOptions, Series,
    TextExt, Widget,
};
use map_model::BusRouteID;

pub struct ActiveTraffic {
    composite: Composite,
}

impl ActiveTraffic {
    pub fn new(ctx: &mut EventCtx, app: &App) -> Box<dyn State> {
        let mut active_agents = vec![Series {
            label: format!("After \"{}\"", app.primary.map.get_edits().edits_name),
            color: app.cs.after_changes,
            pts: app
                .primary
                .sim
                .get_analytics()
                .active_agents(app.primary.sim.time()),
        }];
        if app.has_prebaked().is_some() {
            active_agents.push(Series {
                label: format!("Before \"{}\"", app.primary.map.get_edits().edits_name),
                color: app.cs.before_changes.alpha(0.5),
                pts: app
                    .prebaked()
                    .active_agents(app.primary.sim.get_end_of_day()),
            });
        }

        Box::new(ActiveTraffic {
            composite: Composite::new(Widget::col(vec![
                DashTab::ActiveTraffic.picker(ctx, app),
                LinePlot::new(ctx, active_agents, PlotOptions::fixed()),
            ]))
            .exact_size_percent(90, 90)
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

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::Custom
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        g.clear(app.cs.grass);
        self.composite.draw(g);
    }
}

pub struct TransitRoutes {
    composite: Composite,
}

impl TransitRoutes {
    pub fn new(ctx: &mut EventCtx, app: &App) -> Box<dyn State> {
        // Count totals per route
        let mut boardings = Counter::new();
        for list in app.primary.sim.get_analytics().passengers_boarding.values() {
            for (_, r, _) in list {
                boardings.inc(*r);
            }
        }
        let mut alightings = Counter::new();
        for list in app
            .primary
            .sim
            .get_analytics()
            .passengers_alighting
            .values()
        {
            for (_, r) in list {
                alightings.inc(*r);
            }
        }
        let mut waiting = Counter::new();
        for bs in app.primary.map.all_bus_stops().keys() {
            for (_, r, _, _) in app.primary.sim.get_people_waiting_at_stop(*bs) {
                waiting.inc(*r);
            }
        }

        // Sort descending by count, but ascending by name. Hence the funny negation.
        let mut routes: Vec<(isize, isize, isize, String, BusRouteID)> = Vec::new();
        for r in app.primary.map.all_bus_routes() {
            routes.push((
                -1 * (boardings.get(r.id) as isize),
                -1 * (alightings.get(r.id) as isize),
                -1 * (waiting.get(r.id) as isize),
                r.full_name.clone(),
                r.id,
            ));
        }
        routes.sort();

        let col = vec![
            DashTab::TransitRoutes.picker(ctx, app),
            Line("Transit routes").small_heading().draw(ctx),
            Widget::row(vec![
                Widget::draw_svg(ctx, "system/assets/tools/search.svg"),
                Autocomplete::new(
                    ctx,
                    routes
                        .iter()
                        .map(|(_, _, _, r, id)| (r.clone(), *id))
                        .collect(),
                )
                .named("search"),
            ])
            .padding(8),
            // TODO Maybe a table instead
            Widget::col(
                routes
                    .into_iter()
                    .map(|(boardings, alightings, waiting, name, id)| {
                        Widget::row(vec![
                            Btn::text_fg(name).build(ctx, id.to_string(), None),
                            format!(
                                "{} boardings, {} alightings, {} currently waiting",
                                prettyprint_usize(-boardings as usize),
                                prettyprint_usize(-alightings as usize),
                                prettyprint_usize(-waiting as usize)
                            )
                            .draw_text(ctx),
                        ])
                    })
                    .collect(),
            ),
        ];

        Box::new(TransitRoutes {
            composite: Composite::new(Widget::col(col))
                .exact_size_percent(90, 90)
                .build(ctx),
        })
    }
}

impl State for TransitRoutes {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        let route = match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => {
                if let Some(x) = x.strip_prefix("BusRoute #") {
                    BusRouteID(x.parse::<usize>().unwrap())
                } else {
                    return DashTab::TransitRoutes.transition(ctx, app, &x);
                }
            }
            None => {
                if let Some(routes) = self.composite.autocomplete_done("search") {
                    if !routes.is_empty() {
                        routes[0]
                    } else {
                        return Transition::Keep;
                    }
                } else {
                    return Transition::Keep;
                }
            }
        };

        Transition::PopWithData(Box::new(move |state, ctx, app| {
            let sandbox = state.downcast_mut::<SandboxMode>().unwrap();
            let mut actions = sandbox.contextual_actions();
            sandbox.controls.common.as_mut().unwrap().launch_info_panel(
                ctx,
                app,
                Tab::BusRoute(route),
                &mut actions,
            )
        }))
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::Custom
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        g.clear(app.cs.grass);
        self.composite.draw(g);
    }
}
