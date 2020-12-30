use map_gui::ID;
use map_model::{Map, PathConstraints};
use sim::{AgentID, Sim};
use widgetry::{GfxCtx, Key, Line, Text};

use crate::app::App;

pub struct ObjectDebugger;

impl ObjectDebugger {
    pub fn draw(&self, g: &mut GfxCtx, app: &App) {
        if g.is_key_down(Key::LeftControl) {
            if let Some(pt) = g.canvas.get_cursor_in_map_space() {
                let mut txt = Text::new();
                txt.add(Line(pt.to_string()));
                txt.add(Line(
                    pt.to_gps(app.primary.map.get_gps_bounds()).to_string(),
                ));
                txt.add(Line(format!("{:?}", g.canvas.get_cursor())));
                txt.add(Line(format!("zoom: {}", g.canvas.cam_zoom)));
                g.draw_mouse_tooltip(txt);
            }
        }
    }

    pub fn dump_debug(id: ID, map: &Map, sim: &Sim) {
        match id {
            ID::Lane(id) => {
                let l = map.get_l(id);
                println!("{}", abstutil::to_json(l));

                sim.debug_lane(id);

                let r = map.get_parent(id);
                println!("Parent {} ({}) points to {}", r.id, r.orig_id, r.dst_i);
                println!("{}", abstutil::to_json(r));

                if l.lane_type.is_for_moving_vehicles() {
                    for constraint in vec![
                        PathConstraints::Car,
                        PathConstraints::Bike,
                        PathConstraints::Bus,
                    ] {
                        if constraint.can_use(l, map) {
                            let mut costs = Vec::new();
                            for turn in map.get_turns_to_lane(l.id) {
                                costs.push(map_model::connectivity::driving_cost(
                                    l, turn, constraint, map,
                                ));
                            }
                            println!("Costs for {:?}: {:?}", constraint, costs);
                        }
                    }
                }
            }
            ID::Intersection(id) => {
                let i = map.get_i(id);
                println!("{}", abstutil::to_json(i));

                sim.debug_intersection(id, map);

                println!("{} connecting:", i.orig_id);
                for r in &i.roads {
                    let road = map.get_r(*r);
                    println!("- {} = {}", road.id, road.orig_id);
                }
            }
            ID::Building(id) => {
                println!("{}", abstutil::to_json(map.get_b(id)));
            }
            ID::ParkingLot(id) => {
                println!("{}", abstutil::to_json(map.get_pl(id)));
            }
            ID::Car(id) => {
                sim.debug_car(id);
                if let Some(t) = sim.agent_to_trip(AgentID::Car(id)) {
                    println!("Trip log for {}", t);
                    for p in sim.get_analytics().get_trip_phases(t, map) {
                        println!("- {:?}", p);
                    }
                }
            }
            ID::Pedestrian(id) => {
                sim.debug_ped(id);
                if let Some(t) = sim.agent_to_trip(AgentID::Pedestrian(id)) {
                    println!("Trip log for {}", t);
                    for p in sim.get_analytics().get_trip_phases(t, map) {
                        println!("- {:?}", p);
                    }
                }
            }
            ID::PedCrowd(members) => {
                println!("Crowd with {} members", members.len());
                for p in members {
                    sim.debug_ped(p);
                }
            }
            ID::BusStop(id) => {
                println!("{}", abstutil::to_json(map.get_bs(id)));
            }
            ID::Area(id) => {
                println!("{}", abstutil::to_json(map.get_a(id)));
            }
            ID::Road(_) => unreachable!(),
        }
    }
}
