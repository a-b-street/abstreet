use crate::common::{ColorLegend, Colorer, ColorerBuilder, Warping};
use crate::game::Transition;
use crate::helpers::rotating_color_total;
use crate::helpers::ID;
use crate::managed::{ManagedGUIState, Outcome};
use crate::sandbox::bus_explorer::ShowBusRoute;
use crate::sandbox::SandboxMode;
use crate::ui::UI;
use abstutil::{prettyprint_usize, Counter};
use ezgui::{
    hotkey, Button, Color, Composite, DrawBoth, Drawable, EventCtx, EventLoopMode, GeomBatch,
    GfxCtx, Histogram, HorizontalAlignment, JustDraw, Key, Line, ManagedWidget, Plot, RewriteColor,
    Series, Text, VerticalAlignment,
};
use geom::{Circle, Distance, Duration, PolyLine, Polygon, Pt2D, Statistic, Time};
use map_model::{BusRouteID, IntersectionID};
use sim::ParkingSpot;
use std::collections::HashSet;

pub enum Overlays {
    Inactive,
    ParkingAvailability(Time, Colorer),
    IntersectionDelay(Time, Colorer),
    CumulativeThroughput(Time, Colorer),
    BikeNetwork(Colorer),
    BusNetwork(Colorer),

    FinishedTripsHistogram(Time, Composite),
    IntersectionDemand(Time, IntersectionID, Drawable, Composite),
    BusRoute(Time, BusRouteID, ShowBusRoute),
    BusDelaysOverTime(Time, BusRouteID, Composite),
    BusPassengers(Time, BusRouteID, crate::managed::Composite),
}

impl Overlays {
    pub fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Option<Transition> {
        let now = ui.primary.sim.time();
        match self {
            Overlays::ParkingAvailability(t, _) => {
                if now != *t {
                    *self = Overlays::parking_availability(ctx, ui);
                }
            }
            Overlays::IntersectionDelay(t, _) => {
                if now != *t {
                    *self = Overlays::intersection_delay(ctx, ui);
                }
            }
            Overlays::CumulativeThroughput(t, _) => {
                if now != *t {
                    *self = Overlays::cumulative_throughput(ctx, ui);
                }
            }
            Overlays::IntersectionDemand(t, i, _, _) => {
                if now != *t {
                    *self = Overlays::intersection_demand(*i, ctx, ui);
                }
            }
            Overlays::FinishedTripsHistogram(t, _) => {
                if now != *t {
                    *self = Overlays::finished_trips_histogram(ctx, ui);
                }
            }
            Overlays::BusRoute(t, id, _) => {
                if now != *t {
                    *self = Overlays::show_bus_route(*id, ctx, ui);
                }
            }
            // No updates needed
            Overlays::Inactive | Overlays::BikeNetwork(_) | Overlays::BusNetwork(_) => {}
            Overlays::BusDelaysOverTime(t, id, _) => {
                if now != *t {
                    *self = Overlays::delays_over_time(*id, ctx, ui);
                }
            }
            Overlays::BusPassengers(t, id, _) => {
                if now != *t {
                    *self = Overlays::bus_passengers(*id, ctx, ui);
                }
            }
        };

        match self {
            Overlays::ParkingAvailability(_, ref mut heatmap)
            | Overlays::BikeNetwork(ref mut heatmap)
            | Overlays::BusNetwork(ref mut heatmap)
            | Overlays::IntersectionDelay(_, ref mut heatmap)
            | Overlays::CumulativeThroughput(_, ref mut heatmap) => {
                if heatmap.event(ctx) {
                    *self = Overlays::Inactive;
                }
            }
            Overlays::BusRoute(_, _, ref mut c) => {
                if c.colorer.event(ctx) {
                    *self = Overlays::Inactive;
                }
            }
            Overlays::BusPassengers(_, _, ref mut c) => match c.event(ctx, ui) {
                Some(Outcome::Transition(t)) => {
                    return Some(t);
                }
                Some(Outcome::Clicked(x)) => match x.as_ref() {
                    "X" => {
                        *self = Overlays::Inactive;
                    }
                    _ => unreachable!(),
                },
                None => {}
            },
            Overlays::IntersectionDemand(_, i, _, ref mut c) => match c.event(ctx) {
                Some(ezgui::Outcome::Clicked(x)) => match x.as_ref() {
                    "intersection demand" => {
                        let id = ID::Intersection(*i);
                        return Some(Transition::PushWithMode(
                            Warping::new(
                                ctx,
                                id.canonical_point(&ui.primary).unwrap(),
                                Some(10.0),
                                Some(id.clone()),
                                &mut ui.primary,
                            ),
                            EventLoopMode::Animation,
                        ));
                    }
                    "X" => {
                        *self = Overlays::Inactive;
                    }
                    _ => unreachable!(),
                },
                None => {}
            },
            Overlays::FinishedTripsHistogram(_, ref mut c)
            | Overlays::BusDelaysOverTime(_, _, ref mut c) => match c.event(ctx) {
                Some(ezgui::Outcome::Clicked(x)) => match x.as_ref() {
                    "X" => {
                        *self = Overlays::Inactive;
                    }
                    _ => unreachable!(),
                },
                None => {}
            },
            Overlays::Inactive => {}
        }

        None
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        match self {
            Overlays::Inactive => {}
            Overlays::ParkingAvailability(_, ref heatmap)
            | Overlays::BikeNetwork(ref heatmap)
            | Overlays::BusNetwork(ref heatmap)
            | Overlays::IntersectionDelay(_, ref heatmap)
            | Overlays::CumulativeThroughput(_, ref heatmap) => {
                heatmap.draw(g);
            }
            Overlays::FinishedTripsHistogram(_, ref composite)
            | Overlays::BusDelaysOverTime(_, _, ref composite) => {
                composite.draw(g);
            }
            Overlays::BusPassengers(_, _, ref composite) => {
                composite.draw(g);
            }
            Overlays::IntersectionDemand(_, _, ref draw, ref legend) => {
                g.redraw(draw);
                legend.draw(g);
            }
            Overlays::BusRoute(_, _, ref s) => {
                s.draw(g);
            }
        }
    }

    pub fn maybe_colorer(&self) -> Option<&Colorer> {
        match self {
            Overlays::ParkingAvailability(_, ref heatmap)
            | Overlays::BikeNetwork(ref heatmap)
            | Overlays::BusNetwork(ref heatmap)
            | Overlays::IntersectionDelay(_, ref heatmap)
            | Overlays::CumulativeThroughput(_, ref heatmap) => Some(heatmap),
            Overlays::BusRoute(_, _, ref s) => Some(&s.colorer),
            _ => None,
        }
    }

    pub fn change_overlays(ctx: &mut EventCtx) -> Option<Transition> {
        // TODO Filter out the current
        // TODO Filter out finished trips histogram if prebaked isn't available
        let c = crate::managed::Composite::new(
            Composite::new(
                ManagedWidget::col(vec![
                    ManagedWidget::row(vec![
                        ManagedWidget::draw_text(ctx, Text::from(Line("Heat map layers"))),
                        crate::managed::Composite::text_button(ctx, "X", hotkey(Key::Escape))
                            .align_right(),
                    ]),
                    ManagedWidget::row(vec![
                        crate::managed::Composite::text_button(ctx, "None", hotkey(Key::N)),
                        crate::managed::Composite::text_button(
                            ctx,
                            "finished trips histogram",
                            hotkey(Key::F),
                        ),
                        ManagedWidget::btn(Button::rectangle_svg(
                            "assets/layers/parking_avail.svg",
                            "parking availability",
                            hotkey(Key::P),
                            RewriteColor::ChangeAll(Color::ORANGE),
                            ctx,
                        )),
                        ManagedWidget::btn(Button::rectangle_svg(
                            "assets/layers/intersection_delay.svg",
                            "intersection delay",
                            hotkey(Key::I),
                            RewriteColor::ChangeAll(Color::ORANGE),
                            ctx,
                        )),
                        ManagedWidget::btn(Button::rectangle_svg(
                            "assets/layers/throughput.svg",
                            "throughput",
                            hotkey(Key::T),
                            RewriteColor::ChangeAll(Color::ORANGE),
                            ctx,
                        )),
                        ManagedWidget::btn(Button::rectangle_svg(
                            "assets/layers/bike_network.svg",
                            "bike network",
                            hotkey(Key::B),
                            RewriteColor::ChangeAll(Color::ORANGE),
                            ctx,
                        )),
                        ManagedWidget::btn(Button::rectangle_svg(
                            "assets/layers/bus_network.svg",
                            "bus network",
                            hotkey(Key::U),
                            RewriteColor::ChangeAll(Color::ORANGE),
                            ctx,
                        )),
                    ])
                    .flex_wrap(ctx, 20),
                ])
                .bg(Color::hex("#5B5B5B")),
            )
            .max_size_percent(30, 50)
            .build(ctx),
        )
        .cb("X", Box::new(|_, _| Some(Transition::Pop)))
        .cb(
            "None",
            Box::new(|_, _| {
                Some(Transition::PopWithData(Box::new(|state, _, _| {
                    state.downcast_mut::<SandboxMode>().unwrap().overlay = Overlays::Inactive;
                })))
            }),
        )
        .cb(
            "parking availability",
            Box::new(|_, _| {
                Some(Transition::PopWithData(Box::new(|state, ui, ctx| {
                    state.downcast_mut::<SandboxMode>().unwrap().overlay =
                        Overlays::parking_availability(ctx, ui);
                })))
            }),
        )
        .cb(
            "intersection delay",
            Box::new(|_, _| {
                Some(Transition::PopWithData(Box::new(|state, ui, ctx| {
                    state.downcast_mut::<SandboxMode>().unwrap().overlay =
                        Overlays::intersection_delay(ctx, ui);
                })))
            }),
        )
        .cb(
            "throughput",
            Box::new(|_, _| {
                Some(Transition::PopWithData(Box::new(|state, ui, ctx| {
                    state.downcast_mut::<SandboxMode>().unwrap().overlay =
                        Overlays::cumulative_throughput(ctx, ui);
                })))
            }),
        )
        .cb(
            "bike network",
            Box::new(|_, _| {
                Some(Transition::PopWithData(Box::new(|state, ui, ctx| {
                    state.downcast_mut::<SandboxMode>().unwrap().overlay =
                        Overlays::bike_network(ctx, ui);
                })))
            }),
        )
        .cb(
            "bus network",
            Box::new(|_, _| {
                Some(Transition::PopWithData(Box::new(|state, ui, ctx| {
                    state.downcast_mut::<SandboxMode>().unwrap().overlay =
                        Overlays::bus_network(ctx, ui);
                })))
            }),
        )
        .cb(
            "finished trips histogram",
            Box::new(|_, _| {
                Some(Transition::PopWithData(Box::new(|state, ui, ctx| {
                    state.downcast_mut::<SandboxMode>().unwrap().overlay =
                        Overlays::finished_trips_histogram(ctx, ui);
                })))
            }),
        );
        Some(Transition::Push(ManagedGUIState::over_map(c)))
    }
}

impl Overlays {
    fn parking_availability(ctx: &mut EventCtx, ui: &UI) -> Overlays {
        let (filled_spots, avail_spots) = ui.primary.sim.get_all_parking_spots();
        let mut txt = Text::from(Line("parking availability"));
        txt.add(Line(format!(
            "{} spots filled",
            prettyprint_usize(filled_spots.len())
        )));
        txt.add(Line(format!(
            "{} spots available ",
            prettyprint_usize(avail_spots.len())
        )));

        let awful = Color::hex("#801F1C");
        let bad = Color::hex("#EB5757");
        let meh = Color::hex("#F2C94C");
        let good = Color::hex("#7FFA4D");
        let mut colorer = ColorerBuilder::new(
            txt,
            vec![
                ("< 10%", awful),
                ("< 30%", bad),
                ("< 60%", meh),
                (">= 60%", good),
            ],
        );

        let lane = |spot| match spot {
            ParkingSpot::Onstreet(l, _) => l,
            ParkingSpot::Offstreet(b, _) => ui
                .primary
                .map
                .get_b(b)
                .parking
                .as_ref()
                .unwrap()
                .driving_pos
                .lane(),
        };

        let mut filled = Counter::new();
        let mut avail = Counter::new();
        let mut keys = HashSet::new();
        for spot in filled_spots {
            let l = lane(spot);
            keys.insert(l);
            filled.inc(l);
        }
        for spot in avail_spots {
            let l = lane(spot);
            keys.insert(l);
            avail.inc(l);
        }

        for l in keys {
            let open = avail.get(l);
            let closed = filled.get(l);
            let percent = (open as f64) / ((open + closed) as f64);
            let color = if percent >= 0.6 {
                good
            } else if percent > 0.3 {
                meh
            } else if percent > 0.1 {
                bad
            } else {
                awful
            };
            colorer.add_l(l, color, &ui.primary.map);
        }

        Overlays::ParkingAvailability(ui.primary.sim.time(), colorer.build(ctx, ui))
    }

    pub fn intersection_delay(ctx: &mut EventCtx, ui: &UI) -> Overlays {
        let fast = Color::hex("#7FFA4D");
        let meh = Color::hex("#F4DA22");
        let slow = Color::hex("#EB5757");
        let mut colorer = ColorerBuilder::new(
            Text::from(Line(
                "intersection delay for traffic signals in the last 2 hours (90%ile)",
            )),
            vec![("< 10s", fast), ("<= 60s", meh), ("> 60s", slow)],
        );

        for i in ui.primary.map.all_intersections() {
            let delays = ui.primary.sim.get_analytics().intersection_delays(
                i.id,
                ui.primary.sim.time().clamped_sub(Duration::hours(2)),
                ui.primary.sim.time(),
            );
            if let Some(d) = delays.percentile(90.0) {
                let color = if d < Duration::seconds(10.0) {
                    fast
                } else if d <= Duration::seconds(60.0) {
                    meh
                } else {
                    slow
                };
                colorer.add_i(i.id, color);
            }
        }

        Overlays::IntersectionDelay(ui.primary.sim.time(), colorer.build(ctx, ui))
    }

    fn cumulative_throughput(ctx: &mut EventCtx, ui: &UI) -> Overlays {
        let light = Color::hex("#7FFA4D");
        let medium = Color::hex("#F4DA22");
        let heavy = Color::hex("#EB5757");
        let mut colorer = ColorerBuilder::new(
            Text::from(Line("Throughput")),
            vec![
                ("< 50%ile", light),
                ("< 90%ile", medium),
                (">= 90%ile", heavy),
            ],
        );

        let stats = &ui.primary.sim.get_analytics().thruput_stats;

        // TODO If there are many duplicate counts, arbitrarily some will look heavier! Find the
        // disribution of counts instead.
        // TODO Actually display the counts at these percentiles
        // TODO Dump the data in debug mode
        {
            let roads = stats.count_per_road.sorted_asc();
            let p50_idx = ((roads.len() as f64) * 0.5) as usize;
            let p90_idx = ((roads.len() as f64) * 0.9) as usize;
            for (idx, r) in roads.into_iter().enumerate() {
                let color = if idx < p50_idx {
                    light
                } else if idx < p90_idx {
                    medium
                } else {
                    heavy
                };
                colorer.add_r(*r, color, &ui.primary.map);
            }
        }
        // TODO dedupe
        {
            let intersections = stats.count_per_intersection.sorted_asc();
            let p50_idx = ((intersections.len() as f64) * 0.5) as usize;
            let p90_idx = ((intersections.len() as f64) * 0.9) as usize;
            for (idx, i) in intersections.into_iter().enumerate() {
                let color = if idx < p50_idx {
                    light
                } else if idx < p90_idx {
                    medium
                } else {
                    heavy
                };
                colorer.add_i(*i, color);
            }
        }

        Overlays::CumulativeThroughput(ui.primary.sim.time(), colorer.build(ctx, ui))
    }

    fn bike_network(ctx: &mut EventCtx, ui: &UI) -> Overlays {
        let color = Color::hex("#7FFA4D");
        let mut colorer = ColorerBuilder::new(
            Text::from(Line("bike networks")),
            vec![("bike lanes", color)],
        );
        for l in ui.primary.map.all_lanes() {
            if l.is_biking() {
                colorer.add_l(l.id, color, &ui.primary.map);
            }
        }
        Overlays::BikeNetwork(colorer.build(ctx, ui))
    }

    fn bus_network(ctx: &mut EventCtx, ui: &UI) -> Overlays {
        let lane = Color::hex("#4CA7E9");
        let stop = Color::hex("#4CA7E9");
        let mut colorer = ColorerBuilder::new(
            Text::from(Line("bus networks")),
            vec![("bus lanes", lane), ("bus stops", stop)],
        );
        for l in ui.primary.map.all_lanes() {
            if l.is_bus() {
                colorer.add_l(l.id, lane, &ui.primary.map);
            }
        }
        for bs in ui.primary.map.all_bus_stops().keys() {
            colorer.add_bs(*bs, stop);
        }

        Overlays::BusNetwork(colorer.build(ctx, ui))
    }

    pub fn finished_trips_histogram(ctx: &mut EventCtx, ui: &UI) -> Overlays {
        if !ui.has_prebaked() {
            return Overlays::Inactive;
        }

        let now = ui.primary.sim.time();
        Overlays::FinishedTripsHistogram(
            now,
            Composite::new(
                ManagedWidget::col(vec![
                    ManagedWidget::row(vec![
                        ManagedWidget::draw_text(
                            ctx,
                            Text::from(Line("Are finished trips faster or slower?")),
                        ),
                        crate::managed::Composite::text_button(ctx, "X", None).align_right(),
                    ]),
                    Histogram::new(
                        ui.primary
                            .sim
                            .get_analytics()
                            .finished_trip_deltas(now, ui.prebaked()),
                        ctx,
                    ),
                ])
                .bg(Color::grey(0.4)),
            )
            .aligned(HorizontalAlignment::Right, VerticalAlignment::Center)
            .build(ctx),
        )
    }

    pub fn intersection_demand(i: IntersectionID, ctx: &mut EventCtx, ui: &UI) -> Overlays {
        let mut batch = GeomBatch::new();

        let mut total_demand = 0;
        let mut demand_per_group: Vec<(&PolyLine, usize)> = Vec::new();
        for g in ui.primary.map.get_traffic_signal(i).turn_groups.values() {
            let demand = ui
                .primary
                .sim
                .get_analytics()
                .thruput_stats
                .demand
                .get(&g.id)
                .cloned()
                .unwrap_or(0);
            if demand > 0 {
                total_demand += demand;
                demand_per_group.push((&g.geom, demand));
            }
        }

        for (pl, demand) in demand_per_group {
            let percent = (demand as f64) / (total_demand as f64);
            batch.push(
                Color::RED,
                pl.make_arrow(percent * Distance::meters(5.0)).unwrap(),
            );
        }

        let mut col = vec![ManagedWidget::row(vec![
            ManagedWidget::draw_text(ctx, Text::from(Line("intersection demand"))),
            ManagedWidget::btn(Button::rectangle_svg(
                "assets/tools/locate.svg",
                "intersection demand",
                None,
                RewriteColor::Change(Color::hex("#CC4121"), Color::ORANGE),
                ctx,
            )),
            crate::managed::Composite::text_button(ctx, "X", None).align_right(),
        ])];
        col.push(ColorLegend::row(ctx, Color::RED, "current demand"));

        Overlays::IntersectionDemand(
            ui.primary.sim.time(),
            i,
            batch.upload(ctx),
            Composite::new(ManagedWidget::col(col).bg(Color::grey(0.4)))
                .aligned(HorizontalAlignment::Right, VerticalAlignment::Center)
                .build(ctx),
        )
    }

    pub fn show_bus_route(id: BusRouteID, ctx: &mut EventCtx, ui: &UI) -> Overlays {
        Overlays::BusRoute(ui.primary.sim.time(), id, ShowBusRoute::new(id, ctx, ui))
    }

    pub fn bus_passengers(id: BusRouteID, ctx: &mut EventCtx, ui: &UI) -> Overlays {
        let route = ui.primary.map.get_br(id);
        let mut master_col = vec![ManagedWidget::row(vec![
            ManagedWidget::draw_text(ctx, Text::prompt(&format!("Passengers for {}", route.name))),
            crate::managed::Composite::text_button(ctx, "X", None).align_right(),
        ])];
        let mut col = Vec::new();

        let mut delay_per_stop = ui
            .primary
            .sim
            .get_analytics()
            .bus_passenger_delays(ui.primary.sim.time(), id);
        for idx in 0..route.stops.len() {
            let mut row = vec![
                ManagedWidget::draw_text(ctx, Text::from(Line(format!("Stop {}", idx + 1)))),
                ManagedWidget::btn(Button::rectangle_svg(
                    "assets/tools/locate.svg",
                    &format!("Stop {}", idx + 1),
                    None,
                    RewriteColor::Change(Color::hex("#CC4121"), Color::ORANGE),
                    ctx,
                )),
            ];
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
        batch.push(Color::CYAN, Polygon::rounded_rectangle(15.0, y_len, 4.0));
        for (_, stop_idx, percent_next_stop) in ui.primary.sim.status_of_buses(route.id) {
            // TODO Line it up right in the middle of the line of text. This is probably a bit
            // wrong.
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
        let timeline =
            ManagedWidget::just_draw(JustDraw::wrap(DrawBoth::new(ctx, batch, Vec::new())));

        master_col.push(ManagedWidget::row(vec![
            timeline.margin(5),
            ManagedWidget::col(col).margin(5),
        ]));

        let mut c = crate::managed::Composite::new(
            Composite::new(ManagedWidget::col(master_col).bg(Color::grey(0.4)))
                .aligned(HorizontalAlignment::Right, VerticalAlignment::Center)
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
        Overlays::BusPassengers(ui.primary.sim.time(), id, c)
    }

    pub fn delays_over_time(id: BusRouteID, ctx: &mut EventCtx, ui: &UI) -> Overlays {
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
        Overlays::BusDelaysOverTime(
            ui.primary.sim.time(),
            route.id,
            Composite::new(
                ManagedWidget::col(vec![
                    ManagedWidget::row(vec![
                        ManagedWidget::draw_text(
                            ctx,
                            Text::from(Line(format!("delays for {}", route.name))),
                        ),
                        crate::managed::Composite::text_button(ctx, "X", None).align_right(),
                    ]),
                    Plot::new_duration(series, ctx).margin(10),
                ])
                .bg(Color::grey(0.3)),
            )
            // TODO Doesn't fit
            .aligned(HorizontalAlignment::Right, VerticalAlignment::Center)
            .build(ctx),
        )
    }
}
