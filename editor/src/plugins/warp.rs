use ezgui::{Canvas, TextBox, UserInput};
use map_model::{geometry, AreaID, BuildingID, IntersectionID, LaneID, Map, ParcelID, RoadID};
use objects::ID;
use piston::input::Key;
use plugins::Colorizer;
use sim::{CarID, PedestrianID, Sim};
use std::usize;

pub enum WarpState {
    Empty,
    EnteringSearch(TextBox),
}

impl WarpState {
    pub fn event(
        &mut self,
        input: &mut UserInput,
        map: &Map,
        sim: &Sim,
        canvas: &mut Canvas,
        selected: &mut Option<ID>,
    ) -> bool {
        let mut new_state: Option<WarpState> = None;
        match self {
            WarpState::Empty => {
                if input.unimportant_key_pressed(Key::J, "start searching for something to warp to")
                {
                    new_state = Some(WarpState::EnteringSearch(TextBox::new()));
                }
            }
            WarpState::EnteringSearch(tb) => {
                if tb.event(input.use_event_directly()) {
                    warp(tb.line.clone(), map, sim, canvas, selected);
                    new_state = Some(WarpState::Empty);
                }
                input.consume_event();
            }
        };
        if let Some(s) = new_state {
            *self = s;
        }
        match self {
            WarpState::Empty => false,
            _ => true,
        }
    }

    pub fn get_osd_lines(&self) -> Vec<String> {
        // TODO draw the cursor
        if let WarpState::EnteringSearch(text_box) = self {
            return vec![text_box.line.clone()];
        }
        Vec::new()
    }
}

impl Colorizer for WarpState {}

fn warp(line: String, map: &Map, sim: &Sim, canvas: &mut Canvas, selected: &mut Option<ID>) {
    if line.is_empty() {
        return;
    }

    let pt = match usize::from_str_radix(&line[1..line.len()], 10) {
        // TODO express this more succinctly
        Ok(idx) => match line.chars().next().unwrap() {
            'r' => {
                let id = RoadID(idx);
                if let Some(r) = map.maybe_get_r(id) {
                    let l = map.get_l(r.children_forwards[0].0);
                    println!("Warping to {}, which belongs to {}", l.id, id);
                    *selected = Some(ID::Lane(l.id));
                    l.first_pt()
                } else {
                    println!("{} doesn't exist", id);
                    return;
                }
            }
            'l' => {
                let id = LaneID(idx);
                if let Some(l) = map.maybe_get_l(id) {
                    println!("Warping to {}", id);
                    *selected = Some(ID::Lane(id));
                    l.first_pt()
                } else {
                    println!("{} doesn't exist", id);
                    return;
                }
            }
            'i' => {
                let id = IntersectionID(idx);
                if let Some(i) = map.maybe_get_i(id) {
                    println!("Warping to {}", id);
                    *selected = Some(ID::Intersection(id));
                    i.point
                } else {
                    println!("{} doesn't exist", id);
                    return;
                }
            }
            'b' => {
                let id = BuildingID(idx);
                if let Some(b) = map.maybe_get_b(id) {
                    println!("Warping to {}", id);
                    *selected = Some(ID::Building(id));
                    geometry::center(&b.points)
                } else {
                    println!("{} doesn't exist", id);
                    return;
                }
            }
            'a' => {
                let id = AreaID(idx);
                if let Some(a) = map.maybe_get_a(id) {
                    println!("Warping to {}", id);
                    *selected = Some(ID::Area(id));
                    geometry::center(&a.points)
                } else {
                    println!("{} doesn't exist", id);
                    return;
                }
            }
            // TODO ideally "pa" prefix?
            'e' => {
                let id = ParcelID(idx);
                if let Some(p) = map.maybe_get_p(id) {
                    println!("Warping to {}", id);
                    geometry::center(&p.points)
                } else {
                    println!("{} doesn't exist", id);
                    return;
                }
            }
            'p' => {
                let id = PedestrianID(idx);
                if let Some(p) = sim.get_draw_ped(id, map) {
                    println!("Warping to {}", id);
                    *selected = Some(ID::Pedestrian(id));
                    p.pos
                } else {
                    println!("{} doesn't exist", id);
                    return;
                }
            }
            'c' => {
                let id = CarID(idx);
                if let Some(c) = sim.get_draw_car(id, map) {
                    println!("Warping to {}", id);
                    *selected = Some(ID::Car(id));
                    c.front
                } else {
                    println!("{} doesn't exist", id);
                    return;
                }
            }
            _ => {
                println!("{} isn't a valid ID; Should be [libepc][0-9]+", line);
                return;
            }
        },
        Err(_) => {
            return;
        }
    };
    canvas.center_on_map_pt(pt.x(), pt.y());
}
