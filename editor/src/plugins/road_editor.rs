use ezgui::input::UserInput;
use map_model::{Edits, Map};
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
                    // TODO the magic
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
