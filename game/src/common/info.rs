use crate::common::CommonState;
use crate::game::{State, Transition};
use crate::helpers::ID;
use crate::ui::UI;
use abstutil::prettyprint_usize;
use ezgui::{hotkey, Color, EventCtx, GfxCtx, Key, Line, ModalMenu, Text};
use geom::Duration;
use map_model::PathConstraints;
use sim::CarID;
use std::collections::BTreeMap;

pub struct InfoPanel {
    txt: Text,
    menu: ModalMenu,
}

impl InfoPanel {
    pub fn new(id: ID, ui: &UI, ctx: &EventCtx) -> InfoPanel {
        InfoPanel {
            txt: info_for(id, ui, ctx),
            menu: ModalMenu::new("Info Panel", vec![(hotkey(Key::Escape), "quit")], ctx),
        }
    }
}

impl State for InfoPanel {
    fn event(&mut self, ctx: &mut EventCtx, _: &mut UI) -> Transition {
        self.menu.event(ctx);
        // Can click on the map to cancel
        if self.menu.action("quit")
            || (ctx.input.left_mouse_button_released()
                && ctx.canvas.get_cursor_in_map_space().is_some())
        {
            Transition::Pop
        } else {
            Transition::Keep
        }
    }

    fn draw(&self, g: &mut GfxCtx, _: &UI) {
        g.draw_blocking_text(
            &self.txt,
            (
                ezgui::HorizontalAlignment::Center,
                ezgui::VerticalAlignment::Center,
            ),
        );
        self.menu.draw(g);
    }
}

fn info_for(id: ID, ui: &UI, ctx: &EventCtx) -> Text {
    let (map, sim, draw_map) = (&ui.primary.map, &ui.primary.sim, &ui.primary.draw_map);
    let mut txt = Text::new();
    // TODO Technically we should recalculate all of this as the window resizes, then.
    txt.override_width = Some(0.7 * ctx.canvas.window_width);
    txt.override_height = Some(0.7 * ctx.canvas.window_height);

    txt.extend(&CommonState::default_osd(id.clone(), ui));
    txt.highlight_last_line(Color::BLUE);
    let id_color = ui.cs.get("OSD ID color");
    let name_color = ui.cs.get("OSD name color");

    match id {
        ID::Road(_) => unreachable!(),
        ID::Lane(id) => {
            let l = map.get_l(id);
            let r = map.get_r(l.parent);

            txt.add_appended(vec![
                Line("Parent "),
                Line(r.id.to_string()).fg(id_color),
                Line(" ("),
                Line(r.orig_id.to_string()).fg(id_color),
                Line(" ) points to "),
                Line(r.dst_i.to_string()).fg(id_color),
            ]);
            txt.add(Line(format!(
                "Lane is {} long, parent {} is {} long",
                l.length(),
                r.id,
                r.center_pts.length()
            )));

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

            if l.lane_type.is_for_moving_vehicles() {
                for constraint in vec![
                    PathConstraints::Car,
                    PathConstraints::Bike,
                    PathConstraints::Bus,
                ] {
                    if constraint.can_use(l, map) {
                        txt.add(Line(format!(
                            "Cost for {:?}: {}",
                            constraint,
                            l.get_cost(constraint, map)
                        )));
                    }
                }
            }
        }
        ID::Intersection(id) => {
            let i = map.get_i(id);
            txt.add(Line(i.orig_id.to_string()).fg(id_color));
            txt.add(Line("Connecting"));
            for r in &i.roads {
                let road = map.get_r(*r);
                txt.add_appended(vec![
                    Line("- "),
                    Line(road.get_name()).fg(name_color),
                    Line(" ("),
                    Line(road.id.to_string()).fg(id_color),
                    Line(" = "),
                    Line(road.orig_id.to_string()).fg(id_color),
                    Line(")"),
                ]);
            }

            let delays = ui.primary.sim.get_intersection_delays(id);
            if let Some(p) = delays.percentile(50.0) {
                txt.add(Line(""));
                txt.add(Line(format!("50%ile delay: {}", p)));
            }
            if let Some(p) = delays.percentile(90.0) {
                txt.add(Line(format!("90%ile delay: {}", p)));
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
        // TODO No way to trigger the info panel for this yet.
        ID::Turn(id) => {
            let t = map.get_t(id);
            txt.add(Line(format!("{:?}", t.turn_type)));
        }
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
                let arrivals: Vec<(Duration, CarID)> = all_arrivals
                    .iter()
                    .filter(|(_, _, route, stop)| r.id == *route && id == *stop)
                    .map(|(t, car, _, _)| (*t, *car))
                    .collect();
                if let Some((t, car)) = arrivals.last() {
                    txt.add(Line(format!(
                        "  Last bus arrived {} ago ({})",
                        (sim.time() - *t).minimal_tostring(),
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
