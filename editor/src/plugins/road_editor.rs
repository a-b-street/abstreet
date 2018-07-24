use ezgui::input::UserInput;
use map_model::{EditReason, Edits, LaneType, Map};
use piston::input::Key;
use plugins::selection::SelectionState;

pub enum RoadEditor {
    Inactive(Edits),
    Active(Edits),
}

impl RoadEditor {
    pub fn new(edits: Edits) -> RoadEditor {
        RoadEditor::Inactive(edits)
    }

    pub fn event(
        &mut self,
        input: &mut UserInput,
        map: &Map,
        current_selection: &SelectionState,
    ) -> bool {
        let mut new_state: Option<RoadEditor> = None;
        let active = match self {
            RoadEditor::Inactive(edits) => match current_selection {
                SelectionState::Empty => {
                    if input.unimportant_key_pressed(Key::E, "Start editing roads") {
                        // TODO cloning edits sucks! want to consume self
                        new_state = Some(RoadEditor::Active(edits.clone()));
                        true
                    } else {
                        false
                    }
                }
                _ => false,
            },
            RoadEditor::Active(edits) => {
                if input.key_pressed(Key::Return, "Press enter to stop editing roads") {
                    new_state = Some(RoadEditor::Inactive(edits.clone()));
                } else if let SelectionState::SelectedLane(id, _) = *current_selection {
                    let lane = map.get_l(id);
                    let road = map.get_r(lane.parent);
                    let reason = EditReason::BasemapWrong; // TODO be able to choose

                    // TODO filter out no-ops
                    if input.key_pressed(Key::D, "Press D to make this a driving lane") {
                        if !edits.change_lane_type(reason, road, lane, LaneType::Driving) {
                            println!("Invalid edit");
                        }
                    }
                    if input.key_pressed(Key::P, "Press p to make this a parking lane") {
                        if !edits.change_lane_type(reason, road, lane, LaneType::Parking) {
                            println!("Invalid edit");
                        }
                    }
                    if input.key_pressed(Key::B, "Press b to make this a bike lane") {
                        if !edits.change_lane_type(reason, road, lane, LaneType::Biking) {
                            println!("Invalid edit");
                        }
                    }
                    if input.key_pressed(Key::Backspace, "Press backspace to delete this lane") {
                        if !edits.delete_lane(road, lane) {
                            println!("Invalid edit");
                        }
                    }
                }

                true
            }
        };
        if let Some(s) = new_state {
            *self = s;
        }
        active
    }

    pub fn get_edits(&self) -> &Edits {
        match self {
            RoadEditor::Inactive(edits) => edits,
            RoadEditor::Active(edits) => edits,
        }
    }
}
