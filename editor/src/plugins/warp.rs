use ezgui::canvas::Canvas;
use ezgui::input::UserInput;
use ezgui::text_box::TextBox;
use map_model::{geometry, BuildingID, IntersectionID, LaneID, Map, ParcelID};
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
    let pt = match usize::from_str_radix(&line[1..line.len()], 10) {
        Ok(idx) => match line.chars().next().unwrap() {
            'l' => {
                let id = LaneID(idx);
                println!("Warping to {}", id);
                *selection_state = SelectionState::SelectedLane(id, None);
                map.get_l(id).first_pt()
            }
            'i' => {
                let id = IntersectionID(idx);
                println!("Warping to {}", id);
                *selection_state = SelectionState::SelectedIntersection(id);
                map.get_i(id).point
            }
            'b' => {
                let id = BuildingID(idx);
                println!("Warping to {}", id);
                *selection_state = SelectionState::SelectedBuilding(id);
                geometry::center(&map.get_b(id).points)
            }
            // TODO ideally "pa" prefix?
            'e' => {
                let id = ParcelID(idx);
                println!("Warping to {}", id);
                geometry::center(&map.get_p(id).points)
            }
            'p' => {
                let id = PedestrianID(idx);
                println!("Warping to {}", id);
                sim.get_draw_ped(id, map).focus_pt()
            }
            'c' => {
                let id = CarID(idx);
                println!("Warping to {}", id);
                sim.get_draw_car(id, map).focus_pt()
            }
            _ => {
                println!("{} isn't a valid ID; Should be [libepc][0-9]+", line);
                return;
            }
        },
        Err(_) => {
            println!("{} isn't a valid ID; Should be [libepc][0-9]+", line);
            return;
        }
    };
    canvas.center_on_map_pt(pt.x(), pt.y());
}
