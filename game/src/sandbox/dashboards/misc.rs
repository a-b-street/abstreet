use abstutil::{prettyprint_usize, Counter};
use geom::Time;
use map_model::BusRouteID;
use widgetry::{
    Autocomplete, Btn, EventCtx, GfxCtx, Line, LinePlot, Outcome, Panel, PlotOptions, Series,
    TextExt, Widget,
};

use crate::app::App;
use crate::common::Tab;
use crate::game::{DrawBaselayer, State, Transition};
use crate::sandbox::dashboards::DashTab;
use crate::sandbox::SandboxMode;

pub struct ActiveTraffic {
    panel: Panel,
}

impl ActiveTraffic {
    pub fn new(ctx: &mut EventCtx, app: &App) -> Box<dyn State> {
        // TODO Downsampling in the middle of the day and comparing to the downsampled entire day
        // doesn't work. For the same simulation, by end of day, the plots will be identical, but
        // until then, they'll differ. See https://github.com/dabreegster/abstreet/issues/85 for
        // more details on the "downsampling subset" problem.
        let mut active_agents = vec![Series {
            label: format!("After \"{}\"", app.primary.map.get_edits().edits_name),
            color: app.cs.after_changes,
            pts: downsample(
                app.primary
                    .sim
                    .get_analytics()
                    .active_agents(app.primary.sim.time()),
            ),
        }];
        if app.has_prebaked().is_some() {
            active_agents.push(Series {
                label: format!("Before \"{}\"", app.primary.map.get_edits().edits_name),
                color: app.cs.before_changes.alpha(0.5),
                pts: downsample(
                    app.prebaked()
                        .active_agents(app.primary.sim.get_end_of_day()),
                ),
            });
        }

        Box::new(ActiveTraffic {
            panel: Panel::new(Widget::col(vec![
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
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => Transition::Pop,
                _ => unreachable!(),
            },
            Outcome::Changed => DashTab::ActiveTraffic
                .transition(ctx, app, &self.panel)
                .unwrap(),
            _ => Transition::Keep,
        }
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::Custom
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        g.clear(app.cs.dialog_bg);
        self.panel.draw(g);
    }
}

fn downsample(raw: Vec<(Time, usize)>) -> Vec<(Time, usize)> {
    if raw.is_empty() {
        return raw;
    }

    let min_x = Time::START_OF_DAY;
    let min_y = 0;
    let max_x = raw.last().unwrap().0;
    let max_y = raw.iter().max_by_key(|(_, cnt)| *cnt).unwrap().1;

    let mut pts = Vec::new();
    for (t, cnt) in raw {
        pts.push(lttb::DataPoint::new(
            (t - min_x) / (max_x - min_x),
            ((cnt - min_y) as f64) / ((max_y - min_y) as f64),
        ));
    }
    let mut downsampled = Vec::new();
    for pt in lttb::lttb(pts, 100) {
        downsampled.push((
            max_x.percent_of(pt.x),
            min_y + (pt.y * (max_y - min_y) as f64) as usize,
        ));
    }
    downsampled
}

pub struct TransitRoutes {
    panel: Panel,
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
            Line(format!("{} Transit routes", routes.len()))
                .small_heading()
                .draw(ctx),
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
            panel: Panel::new(Widget::col(col))
                .exact_size_percent(90, 90)
                .build(ctx),
        })
    }
}

impl State for TransitRoutes {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        let route = match self.panel.event(ctx) {
            Outcome::Clicked(x) => {
                if let Some(x) = x.strip_prefix("BusRoute #") {
                    BusRouteID(x.parse::<usize>().unwrap())
                } else if x == "close" {
                    return Transition::Pop;
                } else {
                    unreachable!()
                }
            }
            Outcome::Changed => {
                if let Some(t) = DashTab::TransitRoutes.transition(ctx, app, &self.panel) {
                    return t;
                } else {
                    return Transition::Keep;
                }
            }
            Outcome::Nothing => {
                if let Some(routes) = self.panel.autocomplete_done("search") {
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

        Transition::Multi(vec![
            Transition::Pop,
            Transition::ModifyState(Box::new(move |state, ctx, app| {
                let sandbox = state.downcast_mut::<SandboxMode>().unwrap();
                let mut actions = sandbox.contextual_actions();
                sandbox.controls.common.as_mut().unwrap().launch_info_panel(
                    ctx,
                    app,
                    Tab::BusRoute(route),
                    &mut actions,
                )
            })),
        ])
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::Custom
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        g.clear(app.cs.dialog_bg);
        self.panel.draw(g);
    }
}
