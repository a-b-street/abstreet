use crate::common::CommonState;
use crate::game::{State, Transition};
use crate::helpers::{rotating_color, ID};
use crate::ui::UI;
use abstutil::prettyprint_usize;
use ezgui::{
    hotkey, Color, Composite, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, ManagedWidget,
    ModalMenu, Plot, Series, Text, VerticalAlignment,
};
use geom::{Duration, Statistic, Time};
use map_model::{IntersectionID, RoadID};
use sim::{CarID, TripMode};
use std::collections::BTreeMap;

pub struct InfoPanel {
    composite: Composite,
    menu: ModalMenu,
    actions: Vec<String>,
}

impl InfoPanel {
    pub fn new(id: ID, ui: &mut UI, ctx: &EventCtx) -> InfoPanel {
        let mut menu_entries = vec![(hotkey(Key::Escape), "quit".to_string())];
        let mut actions = Vec::new();
        for (key, label) in ui.per_obj.consume() {
            actions.push(label.clone());
            menu_entries.push((hotkey(key), label));
        }

        let mut col = vec![ManagedWidget::draw_text(ctx, info_for(id.clone(), ui))];

        match id {
            ID::Intersection(i) => {
                if ui.primary.map.get_i(i).is_traffic_signal() {
                    col.push(
                        ManagedWidget::draw_text(ctx, Text::from(Line("delay in 1 hour buckets")))
                            .bg(Color::grey(0.5)),
                    );
                    col.push(
                        intersection_delay(i, Duration::hours(1), ctx, ui)
                            .bg(Color::grey(0.5))
                            .margin(10),
                    );
                }
                col.push(
                    ManagedWidget::draw_text(ctx, Text::from(Line("throughput in 1 hour buckets")))
                        .bg(Color::grey(0.5)),
                );
                col.push(
                    intersection_throughput(i, Duration::hours(1), ctx, ui)
                        .bg(Color::grey(0.5))
                        .margin(10),
                );
            }
            ID::Lane(l) => {
                col.push(
                    ManagedWidget::draw_text(
                        ctx,
                        Text::from(Line("throughput in 1 hour buckets (entire road)")),
                    )
                    .bg(Color::grey(0.5)),
                );
                col.push(
                    road_throughput(ui.primary.map.get_l(l).parent, Duration::hours(1), ctx, ui)
                        .bg(Color::grey(0.5))
                        .margin(10),
                );
            }
            _ => {}
        }

        InfoPanel {
            composite: Composite::aligned(
                ctx,
                (HorizontalAlignment::Left, VerticalAlignment::Top),
                ManagedWidget::col(col).bg(Color::grey(0.3)),
            )
            .scrollable(),
            menu: ModalMenu::new("Info Panel", menu_entries, ctx),
            actions,
        }
    }
}

impl State for InfoPanel {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        self.menu.event(ctx);

        // Can click on the map to cancel
        if self.menu.action("quit")
            || (ctx.canvas.get_cursor_in_map_space().is_some()
                && ui.per_obj.left_click(ctx, "stop showing info"))
        {
            return Transition::Pop;
        }

        for a in &self.actions {
            if self.menu.action(a) {
                return Transition::PopThenApplyObjectAction(a.to_string());
            }
        }

        match self.composite.event(ctx) {
            Some(_) => unreachable!(),
            None => {}
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &UI) {
        self.composite.draw(g);
        self.menu.draw(g);
    }
}

fn info_for(id: ID, ui: &UI) -> Text {
    let (map, sim, draw_map) = (&ui.primary.map, &ui.primary.sim, &ui.primary.draw_map);
    let mut txt = Text::new();

    txt.extend(&CommonState::default_osd(id.clone(), ui));
    txt.highlight_last_line(Color::BLUE);
    let name_color = ui.cs.get("OSD name color");

    match id {
        ID::Road(_) => unreachable!(),
        ID::Lane(id) => {
            let l = map.get_l(id);
            let r = map.get_r(l.parent);

            txt.add(Line(format!("Lane is {} long", l.length())));

            txt.add(Line(""));
            styled_kv(&mut txt, &r.osm_tags);

            txt.add(Line(""));
            if l.is_parking() {
                txt.add(Line(format!(
                    "Has {} parking spots",
                    l.number_parking_spots()
                )));
            } else if l.is_driving() {
                txt.add(Line(format!(
                    "Parking blackhole redirect? {:?}",
                    l.parking_blackhole
                )));
            }

            txt.add(Line(""));
            if let Some(types) = l.get_turn_restrictions(r) {
                txt.add(Line(format!("Turn restriction for this lane: {:?}", types)));
            }
            for (restriction, to) in &r.turn_restrictions {
                txt.add(Line(format!(
                    "Restriction from this road to {}: {:?}",
                    to, restriction
                )));
            }

            txt.add(Line(""));
            txt.add(Line(format!(
                "{} total agents crossed",
                prettyprint_usize(sim.get_analytics().thruput_stats.count_per_road.get(r.id))
            )));
        }
        ID::Intersection(id) => {
            let i = map.get_i(id);
            txt.add(Line("Connecting"));
            for r in &i.roads {
                let road = map.get_r(*r);
                txt.add_appended(vec![Line("- "), Line(road.get_name()).fg(name_color)]);
            }

            let accepted = ui.primary.sim.get_accepted_agents(id);
            if !accepted.is_empty() {
                txt.add(Line(""));
                txt.add(Line(format!("{} turning", accepted.len())));
            }

            let cnt = sim.count_trips_involving_border(id);
            if cnt.nonzero() {
                txt.add(Line(""));
                for line in cnt.describe() {
                    txt.add(Line(line));
                }
            }

            txt.add(Line(""));
            txt.add(Line(format!(
                "{} total agents crossed",
                prettyprint_usize(
                    sim.get_analytics()
                        .thruput_stats
                        .count_per_intersection
                        .get(id)
                )
            )));
        }
        ID::Turn(_) => unreachable!(),
        ID::Building(id) => {
            let b = map.get_b(id);
            txt.add(Line(format!(
                "Dist along sidewalk: {}",
                b.front_path.sidewalk.dist_along()
            )));

            txt.add(Line(""));
            styled_kv(&mut txt, &b.osm_tags);

            if let Some(ref p) = b.parking {
                txt.add(Line(""));
                txt.add_appended(vec![
                    Line(format!("{} parking spots via ", p.num_stalls)),
                    Line(&p.name).fg(name_color),
                ]);
                txt.add(Line(""));
            }

            let cnt = sim.count_trips_involving_bldg(id);
            if cnt.nonzero() {
                txt.add(Line(""));
                for line in cnt.describe() {
                    txt.add(Line(line));
                }
            }

            let cars = sim.get_parked_cars_by_owner(id);
            if !cars.is_empty() {
                txt.add(Line(""));
                txt.add(Line(format!(
                    "{} parked cars owned by this building",
                    cars.len()
                )));
                for p in cars {
                    txt.add(Line(format!("- {}", p.vehicle.id)));
                }
            }
        }
        ID::Car(id) => {
            for line in sim.car_tooltip(id) {
                // TODO Wrap
                txt.add(Line(line));
            }

            // TODO blocked since when
            // TODO dist along trip
            //
            // actions:
            // TODO show route
            // TODO follow
            // TODO jump to src/dst/current spot
        }
        ID::Pedestrian(id) => {
            for line in sim.ped_tooltip(id, map) {
                // TODO Wrap
                txt.add(Line(line));
            }
        }
        ID::PedCrowd(members) => {
            txt.add(Line(format!("Crowd of {}", members.len())));
        }
        ID::ExtraShape(id) => {
            styled_kv(&mut txt, &draw_map.get_es(id).attributes);
        }
        ID::BusStop(id) => {
            let all_arrivals = &sim.get_analytics().bus_arrivals;
            let passengers = &sim.get_analytics().total_bus_passengers;
            for r in map.get_routes_serving_stop(id) {
                txt.add_appended(vec![Line("- Route "), Line(&r.name).fg(name_color)]);
                let arrivals: Vec<(Time, CarID)> = all_arrivals
                    .iter()
                    .filter(|(_, _, route, stop)| r.id == *route && id == *stop)
                    .map(|(t, car, _, _)| (*t, *car))
                    .collect();
                if let Some((t, car)) = arrivals.last() {
                    txt.add(Line(format!(
                        "  Last bus arrived {} ago ({})",
                        sim.time() - *t,
                        car
                    )));
                } else {
                    txt.add(Line("  No arrivals yet"));
                }
                txt.add(Line(format!(
                    "  {} passengers total (any stop)",
                    prettyprint_usize(passengers.get(r.id))
                )));
            }
        }
        ID::Area(id) => {
            let a = map.get_a(id);
            styled_kv(&mut txt, &a.osm_tags);
        }
    };
    txt
}

fn styled_kv(txt: &mut Text, tags: &BTreeMap<String, String>) {
    for (k, v) in tags {
        txt.add_appended(vec![
            Line(k).fg(Color::RED),
            Line(" = "),
            Line(v).fg(Color::CYAN),
        ]);
    }
}

fn intersection_throughput(
    i: IntersectionID,
    bucket: Duration,
    ctx: &EventCtx,
    ui: &UI,
) -> ManagedWidget {
    Plot::new_usize(
        ui.primary
            .sim
            .get_analytics()
            .throughput_intersection(ui.primary.sim.time(), i, bucket)
            .into_iter()
            .map(|(m, pts)| Series {
                label: m.to_string(),
                color: color_for_mode(m, ui),
                pts,
            })
            .collect(),
        ctx,
    )
}

fn road_throughput(r: RoadID, bucket: Duration, ctx: &EventCtx, ui: &UI) -> ManagedWidget {
    Plot::new_usize(
        ui.primary
            .sim
            .get_analytics()
            .throughput_road(ui.primary.sim.time(), r, bucket)
            .into_iter()
            .map(|(m, pts)| Series {
                label: m.to_string(),
                color: color_for_mode(m, ui),
                pts,
            })
            .collect(),
        ctx,
    )
}

fn intersection_delay(
    i: IntersectionID,
    bucket: Duration,
    ctx: &EventCtx,
    ui: &UI,
) -> ManagedWidget {
    let mut series: Vec<(Statistic, Vec<(Time, Duration)>)> = Statistic::all()
        .into_iter()
        .map(|stat| (stat, Vec::new()))
        .collect();
    for (t, distrib) in ui
        .primary
        .sim
        .get_analytics()
        .intersection_delays_bucketized(ui.primary.sim.time(), i, bucket)
    {
        for (stat, pts) in series.iter_mut() {
            if distrib.count() == 0 {
                pts.push((t, Duration::ZERO));
            } else {
                pts.push((t, distrib.select(*stat)));
            }
        }
    }

    Plot::new_duration(
        series
            .into_iter()
            .enumerate()
            .map(|(idx, (stat, pts))| Series {
                label: stat.to_string(),
                color: rotating_color(idx),
                pts,
            })
            .collect(),
        ctx,
    )
}

fn color_for_mode(m: TripMode, ui: &UI) -> Color {
    match m {
        TripMode::Walk => ui.cs.get("unzoomed pedestrian"),
        TripMode::Bike => ui.cs.get("unzoomed bike"),
        TripMode::Transit => ui.cs.get("unzoomed bus"),
        TripMode::Drive => ui.cs.get("unzoomed car"),
    }
}
