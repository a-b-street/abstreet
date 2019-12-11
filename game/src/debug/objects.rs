use crate::helpers::ID;
use crate::render::DrawMap;
use crate::ui::UI;
use ezgui::{EventCtx, GfxCtx, Key, Line, Text};
use map_model::Map;
use sim::{AgentID, CarID, Sim};

pub struct ObjectDebugger {
    debug_tooltip_key_held: bool,
}

impl ObjectDebugger {
    pub fn new() -> ObjectDebugger {
        ObjectDebugger {
            debug_tooltip_key_held: false,
        }
    }

    pub fn event(&mut self, ctx: &mut EventCtx, ui: &UI) {
        if self.debug_tooltip_key_held {
            self.debug_tooltip_key_held = !ctx.input.key_released(Key::RightControl);
        } else {
            self.debug_tooltip_key_held = ctx
                .input
                .unimportant_key_pressed(Key::RightControl, "hold to show debug tooltips");
        }

        if let Some(ref id) = ui.primary.current_selection {
            if ui.per_obj.action(ctx, Key::D, "debug") {
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
        if self.debug_tooltip_key_held {
            if let Some(pt) = g.canvas.get_cursor_in_map_space() {
                if let Some(gps) = pt.to_gps(ui.primary.map.get_gps_bounds()) {
                    let mut txt = Text::new();
                    txt.add(Line(pt.to_string()));
                    txt.add(Line(gps.to_string()));
                    txt.add(Line(format!("{:?}", g.canvas.get_cursor_in_screen_space())));
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
            if let Some(t) = sim.agent_to_trip(AgentID::Car(id)) {
                println!("Trip log for {}", t);
                for p in sim.get_analytics().get_trip_phases(t, map) {
                    println!("- {}", p.describe(sim.time()));
                }
            }
        }
        ID::Pedestrian(id) => {
            sim.debug_ped(id);
            if let Some(t) = sim.agent_to_trip(AgentID::Pedestrian(id)) {
                println!("Trip log for {}", t);
                for p in sim.get_analytics().get_trip_phases(t, map) {
                    println!("- {}", p.describe(sim.time()));
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
            map.get_bs(id).dump_debug();
        }
        ID::Area(id) => {
            map.get_a(id).dump_debug();
        }
    }
}
