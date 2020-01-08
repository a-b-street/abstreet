use crate::common::{edit_map_panel, Warping};
use crate::game::{msg, Transition, WizardState};
use crate::helpers::{rotating_color_total, ID};
use crate::sandbox::gameplay::{
    cmp_duration_shorter, manage_overlays, GameplayMode, GameplayState,
};
use crate::sandbox::overlays::Overlays;
use crate::sandbox::{bus_explorer, SandboxMode};
use crate::ui::UI;
use ezgui::{
    hotkey, Button, Choice, Color, Composite, DrawBoth, EventCtx, EventLoopMode, GeomBatch,
    HorizontalAlignment, JustDraw, Key, Line, ManagedWidget, ModalMenu, Plot, Series, Text,
    VerticalAlignment,
};
use geom::{Circle, Distance, Polygon, Pt2D, Statistic, Time};
use map_model::BusRouteID;

pub struct OptimizeBus {
    route: BusRouteID,
    time: Time,
    stat: Statistic,
}

impl OptimizeBus {
    pub fn new(
        route_name: String,
        ctx: &mut EventCtx,
        ui: &UI,
    ) -> (ModalMenu, crate::managed::Composite, Box<dyn GameplayState>) {
        let route = ui.primary.map.get_bus_route(&route_name).unwrap();
        (
            ModalMenu::new(
                format!("Optimize {}", route_name),
                vec![
                    (hotkey(Key::E), "show bus route"),
                    (hotkey(Key::T), "show delays over time"),
                    (hotkey(Key::P), "show bus passengers"),
                    (hotkey(Key::S), "change statistic"),
                    (hotkey(Key::H), "help"),
                ],
                ctx,
            ),
            edit_map_panel(ctx, ui, GameplayMode::OptimizeBus(route_name.clone())),
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
        if manage_overlays(
            menu,
            ctx,
            "show bus passengers",
            "hide bus passengers",
            overlays,
            match overlays {
                Overlays::BusPassengers(_) => true,
                _ => false,
            },
            self.time != ui.primary.sim.time(),
        ) {
            *overlays = Overlays::BusPassengers(bus_passengers(self.route, ui, ctx));
        }

        // TODO Expensive
        if self.time != ui.primary.sim.time() {
            self.time = ui.primary.sim.time();
            menu.set_info(ctx, bus_route_panel(self.route, self.stat, ui));
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

fn bus_route_panel(id: BusRouteID, stat: Statistic, ui: &UI) -> Text {
    let now = ui
        .primary
        .sim
        .get_analytics()
        .bus_arrivals(ui.primary.sim.time(), id);
    let baseline = ui.prebaked().bus_arrivals(ui.primary.sim.time(), id);

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

fn bus_passengers(id: BusRouteID, ui: &UI, ctx: &mut EventCtx) -> crate::managed::Composite {
    let route = ui.primary.map.get_br(id);
    let mut master_col = vec![ManagedWidget::draw_text(
        ctx,
        Text::prompt(&format!("Passengers for {}", route.name)),
    )];
    let mut col = Vec::new();

    let mut delay_per_stop = ui
        .primary
        .sim
        .get_analytics()
        .bus_passenger_delays(ui.primary.sim.time(), id);
    for idx in 0..route.stops.len() {
        let mut row = vec![ManagedWidget::btn(Button::text_no_bg(
            Text::from(Line(format!("Stop {}", idx + 1))),
            Text::from(Line(format!("Stop {}", idx + 1)).fg(Color::ORANGE)),
            None,
            &format!("Stop {}", idx + 1),
            ctx,
        ))];
        if let Some(hgram) = delay_per_stop.remove(&route.stops[idx]) {
            row.push(ManagedWidget::draw_text(
                ctx,
                Text::from(Line(format!(
                    ": {} (avg {})",
                    hgram.count(),
                    hgram.select(Statistic::Mean)
                ))),
            ));
        } else {
            row.push(ManagedWidget::draw_text(ctx, Text::from(Line(": nobody"))));
        }
        col.push(ManagedWidget::row(row));
    }

    let y_len = ctx.default_line_height() * (route.stops.len() as f64);
    let mut batch = GeomBatch::new();
    batch.push(
        Color::CYAN,
        Polygon::rounded_rectangle(
            Distance::meters(15.0),
            Distance::meters(y_len),
            Distance::meters(4.0),
        ),
    );
    for (_, stop_idx, percent_next_stop) in ui.primary.sim.status_of_buses(route.id) {
        // TODO Line it up right in the middle of the line of text. This is probably a bit wrong.
        let base_percent_y = if stop_idx == route.stops.len() - 1 {
            0.0
        } else {
            (stop_idx as f64) / ((route.stops.len() - 1) as f64)
        };
        batch.push(
            Color::BLUE,
            Circle::new(
                Pt2D::new(
                    7.5,
                    base_percent_y * y_len + percent_next_stop * ctx.default_line_height(),
                ),
                Distance::meters(5.0),
            )
            .to_polygon(),
        );
    }
    let timeline = ManagedWidget::just_draw(JustDraw::wrap(DrawBoth::new(ctx, batch, Vec::new())));

    master_col.push(ManagedWidget::row(vec![
        timeline.margin(5),
        ManagedWidget::col(col).margin(5),
    ]));

    let mut c = crate::managed::Composite::new(
        Composite::new(ManagedWidget::col(master_col).bg(Color::grey(0.4)))
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Center)
            .build(ctx),
    );
    for (idx, stop) in route.stops.iter().enumerate() {
        let id = ID::BusStop(*stop);
        c = c.cb(
            &format!("Stop {}", idx + 1),
            Box::new(move |ctx, ui| {
                Some(Transition::PushWithMode(
                    Warping::new(
                        ctx,
                        id.canonical_point(&ui.primary).unwrap(),
                        Some(4.0),
                        Some(id.clone()),
                        &mut ui.primary,
                    ),
                    EventLoopMode::Animation,
                ))
            }),
        );
    }
    c
}

fn bus_delays(id: BusRouteID, ui: &UI, ctx: &mut EventCtx) -> Composite {
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
    Composite::new(
        ManagedWidget::col(vec![
            ManagedWidget::draw_text(ctx, Text::from(Line(format!("delays for {}", route.name)))),
            Plot::new_duration(series, ctx).margin(10),
        ])
        .bg(Color::grey(0.3)),
    )
    .aligned(HorizontalAlignment::Center, VerticalAlignment::Center)
    .build(ctx)
}
