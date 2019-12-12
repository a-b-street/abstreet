use crate::common::{Plot, Series};
use crate::game::{msg, Transition, WizardState};
use crate::helpers::rotating_color_total;
use crate::sandbox::gameplay::{cmp_duration_shorter, manage_overlays, GameplayState};
use crate::sandbox::overlays::Overlays;
use crate::sandbox::{bus_explorer, SandboxMode};
use crate::ui::UI;
use ezgui::{hotkey, Choice, EventCtx, Key, Line, ModalMenu, Text};
use geom::{Duration, Statistic, Time};
use map_model::BusRouteID;
use sim::Analytics;

pub struct OptimizeBus {
    route: BusRouteID,
    time: Time,
    stat: Statistic,
}

impl OptimizeBus {
    pub fn new(route_name: String, ctx: &EventCtx, ui: &UI) -> (ModalMenu, Box<dyn GameplayState>) {
        let route = ui.primary.map.get_bus_route(&route_name).unwrap();
        (
            ModalMenu::new(
                format!("Optimize {}", route_name),
                vec![
                    (hotkey(Key::E), "show bus route"),
                    (hotkey(Key::T), "show delays over time"),
                    (hotkey(Key::S), "change statistic"),
                    (hotkey(Key::H), "help"),
                ],
                ctx,
            ),
            Box::new(OptimizeBus {
                route: route.id,
                time: Time::START_OF_DAY,
                stat: Statistic::Max,
            }),
        )
    }
}

impl GameplayState for OptimizeBus {
    fn event(
        &mut self,
        ctx: &mut EventCtx,
        ui: &mut UI,
        overlays: &mut Overlays,
        prebaked: &Analytics,
        menu: &mut ModalMenu,
    ) -> Option<Transition> {
        menu.event(ctx);
        if manage_overlays(
            menu,
            ctx,
            "show bus route",
            "hide bus route",
            overlays,
            match overlays {
                Overlays::BusRoute(_) => true,
                _ => false,
            },
            self.time != ui.primary.sim.time(),
        ) {
            *overlays = Overlays::BusRoute(bus_explorer::ShowBusRoute::new(
                ui.primary.map.get_br(self.route),
                ui,
                ctx,
            ));
        }
        if manage_overlays(
            menu,
            ctx,
            "show delays over time",
            "hide delays over time",
            overlays,
            match overlays {
                Overlays::BusDelaysOverTime(_) => true,
                _ => false,
            },
            self.time != ui.primary.sim.time(),
        ) {
            *overlays = Overlays::BusDelaysOverTime(bus_delays(self.route, ui, ctx));
        }

        // TODO Expensive
        if self.time != ui.primary.sim.time() {
            self.time = ui.primary.sim.time();
            menu.set_info(ctx, bus_route_panel(self.route, self.stat, ui, prebaked));
        }

        if menu.action("change statistic") {
            return Some(Transition::Push(WizardState::new(Box::new(
                move |wiz, ctx, _| {
                    // TODO Filter out existing. Make this kind of thing much easier.
                    let (_, new_stat) = wiz.wrap(ctx).choose(
                        "Show which statistic on frequency a bus stop is visited?",
                        || {
                            Statistic::all()
                                .into_iter()
                                .map(|s| Choice::new(s.to_string(), s))
                                .collect()
                        },
                    )?;
                    Some(Transition::PopWithData(Box::new(move |state, _, _| {
                        let sandbox = state.downcast_mut::<SandboxMode>().unwrap();
                        let opt = sandbox
                            .gameplay
                            .state
                            .downcast_mut::<OptimizeBus>()
                            .unwrap();
                        // Force recalculation
                        opt.time = Time::START_OF_DAY;
                        opt.stat = new_stat;
                    })))
                },
            ))));
        }
        if menu.action("help") {
            return Some(Transition::Push(msg(
                "Help",
                vec![
                    "First find where the bus gets stuck.",
                    "Then use edit mode to try to speed things up.",
                    "Try making dedicated bus lanes",
                    "and adjusting traffic signals.",
                ],
            )));
        }
        None
    }
}

fn bus_route_panel(id: BusRouteID, stat: Statistic, ui: &UI, prebaked: &Analytics) -> Text {
    let now = ui
        .primary
        .sim
        .get_analytics()
        .bus_arrivals(ui.primary.sim.time(), id);
    let baseline = prebaked.bus_arrivals(ui.primary.sim.time(), id);

    let route = ui.primary.map.get_br(id);
    let mut txt = Text::new();
    txt.add(Line(format!("{} delay between stops", stat)));
    for idx1 in 0..route.stops.len() {
        let idx2 = if idx1 == route.stops.len() - 1 {
            0
        } else {
            idx1 + 1
        };
        // TODO Also display number of arrivals...
        txt.add(Line(format!("Stop {}->{}: ", idx1 + 1, idx2 + 1)));
        if let Some(ref stats1) = now.get(&route.stops[idx2]) {
            let a = stats1.select(stat);
            txt.append(Line(a.to_string()));

            if let Some(ref stats2) = baseline.get(&route.stops[idx2]) {
                txt.append_all(cmp_duration_shorter(a, stats2.select(stat)));
            }
        } else {
            txt.append(Line("no arrivals yet"));
        }
    }
    txt
}

fn bus_delays(id: BusRouteID, ui: &UI, ctx: &mut EventCtx) -> Plot<Duration> {
    let route = ui.primary.map.get_br(id);
    let mut delays_per_stop = ui
        .primary
        .sim
        .get_analytics()
        .bus_arrivals_over_time(ui.primary.sim.time(), id);

    let mut series = Vec::new();
    for idx1 in 0..route.stops.len() {
        let idx2 = if idx1 == route.stops.len() - 1 {
            0
        } else {
            idx1 + 1
        };
        series.push(Series {
            label: format!("Stop {}->{}", idx1 + 1, idx2 + 1),
            color: rotating_color_total(idx1, route.stops.len()),
            pts: delays_per_stop
                .remove(&route.stops[idx2])
                .unwrap_or_else(Vec::new),
        });
    }
    Plot::new(
        &format!("delays for {}", route.name),
        series,
        Duration::ZERO,
        ctx,
    )
}
