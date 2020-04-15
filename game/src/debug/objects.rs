use crate::app::App;
use crate::helpers::ID;
use crate::render::DrawMap;
use ezgui::{EventCtx, GfxCtx, Key, Line, Text};
use map_model::{Map, PathConstraints};
use sim::{AgentID, Sim};

pub struct ObjectDebugger {
    debug_tooltip_key_held: bool,
}

impl ObjectDebugger {
    pub fn new() -> ObjectDebugger {
        ObjectDebugger {
            debug_tooltip_key_held: false,
        }
    }

    pub fn event(&mut self, ctx: &mut EventCtx) {
        if self.debug_tooltip_key_held {
            self.debug_tooltip_key_held = !ctx.input.key_released(Key::RightControl);
        } else {
            self.debug_tooltip_key_held = ctx
                .input
                .unimportant_key_pressed(Key::RightControl, "hold to show debug tooltips");
        }
    }

    pub fn draw(&self, g: &mut GfxCtx, app: &App) {
        if self.debug_tooltip_key_held {
            if let Some(pt) = g.canvas.get_cursor_in_map_space() {
                if let Some(gps) = pt.to_gps(app.primary.map.get_gps_bounds()) {
                    let mut txt = Text::new();
                    txt.add(Line(pt.to_string()));
                    txt.add(Line(gps.to_string()));
                    txt.add(Line(format!("{:?}", g.canvas.get_cursor())));
                    txt.add(Line(format!("zoom: {}", g.canvas.cam_zoom)));
                    g.draw_mouse_tooltip(txt);
                }
            }
        }
    }

    pub fn dump_debug(id: ID, map: &Map, sim: &Sim, draw_map: &DrawMap) {
        match id {
            ID::Lane(id) => {
                let l = map.get_l(id);
                println!("{}", abstutil::to_json(l));

                sim.debug_lane(id);

                let r = map.get_parent(id);
                println!("Parent {} ({}) points to {}", r.id, r.orig_id, r.dst_i);

                if l.lane_type.is_for_moving_vehicles() {
                    for constraint in vec![
                        PathConstraints::Car,
                        PathConstraints::Bike,
                        PathConstraints::Bus,
                    ] {
                        if constraint.can_use(l, map) {
                            println!(
                                "Cost for {:?}: {}",
                                constraint,
                                l.get_max_cost(constraint, map)
                            );
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
            ID::Turn(id) => {
                println!("{}", abstutil::to_json(map.get_t(id)));
            }
            ID::Building(id) => {
                println!("{}", abstutil::to_json(map.get_b(id)));
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
            ID::ExtraShape(id) => {
                let es = draw_map.get_es(id);
                for (k, v) in &es.attributes {
                    println!("{} = {}", k, v);
                }
                println!("associated road: {:?}", es.road);
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
