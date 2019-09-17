use crate::helpers::ID;
use crate::render::DrawMap;
use crate::ui::PerMapUI;
use crate::ui::UI;
use ezgui::{Color, EventCtx, GfxCtx, Key, Line, Text};
use map_model::raw_data::StableRoadID;
use map_model::Map;
use sim::{CarID, Sim};
use std::collections::BTreeMap;

pub struct ObjectDebugger {
    tooltip_key_held: bool,
    debug_tooltip_key_held: bool,
    selected: Option<ID>,
}

impl ObjectDebugger {
    pub fn new() -> ObjectDebugger {
        ObjectDebugger {
            tooltip_key_held: false,
            debug_tooltip_key_held: false,
            selected: None,
        }
    }

    pub fn event(&mut self, ctx: &mut EventCtx, ui: &UI) {
        self.selected = ui.primary.current_selection.clone();
        if self.tooltip_key_held {
            self.tooltip_key_held = !ctx.input.key_released(Key::LeftControl);
        } else {
            // TODO Can't really display an OSD action if we're not currently selecting something.
            // Could only activate sometimes, but that seems a bit harder to use.
            self.tooltip_key_held = ctx
                .input
                .unimportant_key_pressed(Key::LeftControl, "hold to show tooltips");
        }
        if self.debug_tooltip_key_held {
            self.debug_tooltip_key_held = !ctx.input.key_released(Key::RightControl);
        } else {
            self.debug_tooltip_key_held = ctx
                .input
                .unimportant_key_pressed(Key::RightControl, "hold to show debug tooltips");
        }

        if let Some(ref id) = self.selected {
            if ctx.input.contextual_action(Key::D, "debug") {
                dump_debug(
                    id.clone(),
                    &ui.primary.map,
                    &ui.primary.sim,
                    &ui.primary.draw_map,
                );
            }
        }
    }

    pub fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        if self.tooltip_key_held {
            if let Some(ref id) = self.selected {
                let txt = tooltip_lines(id.clone(), g, &ui.primary);
                g.draw_mouse_tooltip(&txt);
            }
        }

        if self.debug_tooltip_key_held {
            if let Some(pt) = g.canvas.get_cursor_in_map_space() {
                if let Some(gps) = pt.to_gps(ui.primary.map.get_gps_bounds()) {
                    let mut txt = Text::new();
                    txt.add(Line(pt.to_string()));
                    txt.add(Line(gps.to_string()));
                    txt.add(Line(format!("zoom: {}", g.canvas.cam_zoom)));
                    g.draw_mouse_tooltip(&txt);
                }
            }
        }
    }
}

fn dump_debug(id: ID, map: &Map, sim: &Sim, draw_map: &DrawMap) {
    match id {
        ID::Road(id) => {
            map.get_r(id).dump_debug();
        }
        ID::Lane(id) => {
            map.get_l(id).dump_debug();
            sim.debug_lane(id);
        }
        ID::Intersection(id) => {
            map.get_i(id).dump_debug();
            sim.debug_intersection(id, map);
        }
        ID::Turn(id) => {
            map.get_t(id).dump_debug();
        }
        ID::Building(id) => {
            map.get_b(id).dump_debug();
            for (cars, descr) in vec![
                (
                    sim.get_parked_cars_by_owner(id),
                    format!("currently parked cars are owned by {}", id),
                ),
                (
                    sim.get_offstreet_parked_cars(id),
                    format!("cars are parked inside {}", id),
                ),
            ] {
                println!(
                    "{} {}: {:?}",
                    cars.len(),
                    descr,
                    cars.iter().map(|p| p.vehicle.id).collect::<Vec<CarID>>()
                );
            }
        }
        ID::Car(id) => {
            sim.debug_car(id);
        }
        ID::Pedestrian(id) => {
            sim.debug_ped(id);
        }
        ID::PedCrowd(members) => {
            println!("Crowd with {} members", members.len());
            for p in members {
                sim.debug_ped(p);
            }
        }
        ID::ExtraShape(id) => {
            let es = draw_map.get_es(id);
            for (k, v) in &es.attributes {
                println!("{} = {}", k, v);
            }
            println!("associated road: {:?}", es.road);
        }
        ID::BusStop(id) => {
            map.get_bs(id).dump_debug();
        }
        ID::Area(id) => {
            map.get_a(id).dump_debug();
        }
        ID::Trip(id) => {
            sim.debug_trip(id);
        }
    }
}

fn tooltip_lines(id: ID, g: &mut GfxCtx, ctx: &PerMapUI) -> Text {
    let (map, sim, draw_map) = (&ctx.map, &ctx.sim, &ctx.draw_map);
    let mut txt = Text::new();
    match id {
        ID::Road(id) => {
            let r = map.get_r(id);
            txt.add_appended(vec![
                Line(format!("{} (originally {}) is ", r.id, r.stable_id)),
                Line(r.get_name()).fg(Color::CYAN),
            ]);
            txt.add(Line(format!("From OSM way {}", r.osm_way_id)));
        }
        ID::Lane(id) => {
            let l = map.get_l(id);
            let r = map.get_r(l.parent);

            txt.add_appended(vec![
                Line(format!("{} is ", l.id)),
                Line(r.get_name()).fg(Color::CYAN),
            ]);
            txt.add(Line(format!(
                "Parent {} (originally {}) points to {}",
                r.id, r.stable_id, r.dst_i
            )));
            txt.add(Line(format!(
                "Lane is {} long, parent {} is {} long",
                l.length(),
                r.id,
                r.center_pts.length()
            )));
            styled_kv(&mut txt, &r.osm_tags);
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
            if let Some(types) = l.get_turn_restrictions(r) {
                txt.add(Line(format!("Turn restriction for this lane: {:?}", types)));
            }
            for (restriction, to) in &r.turn_restrictions {
                txt.add(Line(format!(
                    "Restriction from this road to {}: {}",
                    to, restriction
                )));
            }
        }
        ID::Intersection(id) => {
            txt.add(Line(id.to_string()));
            let i = map.get_i(id);
            txt.add(Line(format!("Roads: {:?}", i.roads)));
            txt.add(Line(format!(
                "Orig roads: {:?}",
                i.roads
                    .iter()
                    .map(|r| map.get_r(*r).stable_id)
                    .collect::<Vec<StableRoadID>>()
            )));
            txt.add(Line(format!("Originally {}", i.stable_id)));
        }
        ID::Turn(id) => {
            let t = map.get_t(id);
            txt.add(Line(format!("{}", id)));
            txt.add(Line(format!("{:?}", t.turn_type)));
        }
        ID::Building(id) => {
            let b = map.get_b(id);
            txt.add(Line(format!("Building #{:?}", id)));
            txt.add(Line(format!(
                "Dist along sidewalk: {}",
                b.front_path.sidewalk.dist_along()
            )));
            styled_kv(&mut txt, &b.osm_tags);
        }
        ID::Car(id) => {
            for line in sim.car_tooltip(id) {
                txt.add_wrapped_line(&g.canvas, line);
            }
        }
        ID::Pedestrian(id) => {
            for line in sim.ped_tooltip(id) {
                txt.add_wrapped_line(&g.canvas, line);
            }
        }
        ID::PedCrowd(members) => {
            txt.add(Line(format!("Crowd of {}", members.len())));
        }
        ID::ExtraShape(id) => {
            styled_kv(&mut txt, &draw_map.get_es(id).attributes);
        }
        ID::BusStop(id) => {
            txt.add(Line(id.to_string()));
            for r in map.get_all_bus_routes() {
                if r.stops.contains(&id) {
                    txt.add(Line(format!("- Route {}", r.name)));
                }
            }
        }
        ID::Area(id) => {
            let a = map.get_a(id);
            txt.add(Line(format!("{}", id)));
            styled_kv(&mut txt, &a.osm_tags);
        }
        ID::Trip(_) => {}
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
