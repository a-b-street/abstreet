use ezgui::canvas::Canvas;
use ezgui::input::UserInput;
use ezgui::text_box::TextBox;
use map_model::{geometry, BuildingID, IntersectionID, LaneID, Map, ParcelID, RoadID};
use piston::input::Key;
use plugins::selection::SelectionState;
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
        selection_state: &mut SelectionState,
    ) -> bool {
        let mut new_state: Option<WarpState> = None;
        let active = match self {
            WarpState::Empty => {
                if input.unimportant_key_pressed(Key::J, "start searching for something to warp to")
                {
                    new_state = Some(WarpState::EnteringSearch(TextBox::new()));
                    true
                } else {
                    false
                }
            }
            WarpState::EnteringSearch(tb) => {
                if tb.event(input.use_event_directly()) {
                    warp(tb.line.clone(), map, sim, canvas, selection_state);
                    new_state = Some(WarpState::Empty);
                }
                input.consume_event();
                true
            }
        };
        if let Some(s) = new_state {
            *self = s;
        }
        active
    }

    pub fn get_osd_lines(&self) -> Vec<String> {
        // TODO draw the cursor
        if let WarpState::EnteringSearch(text_box) = self {
            return vec![text_box.line.clone()];
        }
        Vec::new()
    }
}

fn warp(
    line: String,
    map: &Map,
    sim: &Sim,
    canvas: &mut Canvas,
    selection_state: &mut SelectionState,
) {
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
                    *selection_state = SelectionState::SelectedLane(l.id, None);
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
                    *selection_state = SelectionState::SelectedLane(id, None);
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
                    *selection_state = SelectionState::SelectedIntersection(id);
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
                    *selection_state = SelectionState::SelectedBuilding(id);
                    geometry::center(&b.points)
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
                    *selection_state = SelectionState::SelectedPedestrian(id);
                    p.focus_pt()
                } else {
                    println!("{} doesn't exist", id);
                    return;
                }
            }
            'c' => {
                let id = CarID(idx);
                if let Some(c) = sim.get_draw_car(id, map) {
                    println!("Warping to {}", id);
                    *selection_state = SelectionState::SelectedCar(id);
                    c.focus_pt()
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
