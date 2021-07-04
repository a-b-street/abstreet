use map_gui::ID;
use map_model::{Map, Position};
use sim::{AgentID, Sim};
use widgetry::{GfxCtx, Key, Text};

use crate::app::App;

pub struct ObjectDebugger;

impl ObjectDebugger {
    pub fn draw(&self, g: &mut GfxCtx, app: &App) {
        if g.is_key_down(Key::LeftControl) {
            if let Some(pt) = g.canvas.get_cursor_in_map_space() {
                let mut txt = Text::new();
                txt.add_line(pt.to_string());
                txt.add_line(pt.to_gps(app.primary.map.get_gps_bounds()).to_string());
                txt.add_line(format!("{:?}", g.canvas.get_cursor()));
                txt.add_line(format!("zoom: {}", g.canvas.cam_zoom));
                txt.add_line(format!(
                    "cam_x = {}, cam_y = {}",
                    g.canvas.cam_x, g.canvas.cam_y
                ));
                if let Some(ID::Lane(l)) = app.primary.current_selection {
                    let pl = &app.primary.map.get_l(l).lane_center_pts;
                    if let Some((dist, _)) = pl.dist_along_of_point(pl.project_pt(pt)) {
                        txt.add_line(Position::new(l, dist).to_string());
                    }
                }
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
