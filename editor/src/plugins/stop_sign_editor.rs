// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use colors::{ColorScheme, Colors};
use control::stop_signs::TurnPriority;
use control::ControlMap;
use ezgui::UserInput;
use graphics::types::Color;
use map_model::IntersectionID;
use map_model::{Map, Turn};
use objects::ID;
use piston::input::Key;

#[derive(PartialEq)]
pub enum StopSignEditor {
    Inactive,
    Active(IntersectionID),
}

impl StopSignEditor {
    pub fn new() -> StopSignEditor {
        StopSignEditor::Inactive
    }

    pub fn event(
        &mut self,
        input: &mut UserInput,
        map: &Map,
        control_map: &mut ControlMap,
        selected: Option<ID>,
    ) -> bool {
        if *self == StopSignEditor::Inactive {
            match selected {
                Some(ID::Intersection(id)) => {
                    if control_map.stop_signs.contains_key(&id)
                        && input.key_pressed(Key::E, &format!("edit stop signs for {}", id))
                    {
                        *self = StopSignEditor::Active(id);
                        return true;
                    }
                }
                _ => {}
            }
        }

        let mut new_state: Option<StopSignEditor> = None;
        match self {
            StopSignEditor::Inactive => {}
            StopSignEditor::Active(i) => {
                if input.key_pressed(Key::Return, "quit the editor") {
                    new_state = Some(StopSignEditor::Inactive);
                } else if let Some(ID::Turn(id)) = selected {
                    if map.get_t(id).parent == *i {
                        let sign = &mut control_map.stop_signs.get_mut(i).unwrap();
                        match sign.get_priority(id) {
                            TurnPriority::Priority => {
                                if input.key_pressed(Key::D2, "make this turn yield") {
                                    sign.set_priority(id, TurnPriority::Yield, map);
                                }
                                if input.key_pressed(Key::D3, "make this turn always stop") {
                                    sign.set_priority(id, TurnPriority::Stop, map);
                                }
                            }
                            TurnPriority::Yield => {
                                if sign.could_be_priority_turn(id, map)
                                    && input.key_pressed(Key::D1, "let this turn go immediately")
                                {
                                    sign.set_priority(id, TurnPriority::Priority, map);
                                }
                                if input.key_pressed(Key::D3, "make this turn always stop") {
                                    sign.set_priority(id, TurnPriority::Stop, map);
                                }
                            }
                            TurnPriority::Stop => {
                                if sign.could_be_priority_turn(id, map)
                                    && input.key_pressed(Key::D1, "let this turn go immediately")
                                {
                                    sign.set_priority(id, TurnPriority::Priority, map);
                                }
                                if input.key_pressed(Key::D2, "make this turn yield") {
                                    sign.set_priority(id, TurnPriority::Yield, map);
                                }
                            }
                        };
                    }
                }
            }
        };
        if let Some(s) = new_state {
            *self = s;
        }

        match self {
            StopSignEditor::Inactive => false,
            _ => true,
        }
    }

    pub fn color_t(&self, t: &Turn, control_map: &ControlMap, cs: &ColorScheme) -> Option<Color> {
        match self {
            StopSignEditor::Inactive => None,
            StopSignEditor::Active(i) => {
                if t.parent != *i {
                    return Some(cs.get(Colors::TurnIrrelevant));
                }
                match control_map.stop_signs[i].get_priority(t.id) {
                    TurnPriority::Priority => Some(cs.get(Colors::PriorityTurn)),
                    TurnPriority::Yield => Some(cs.get(Colors::YieldTurn)),
                    TurnPriority::Stop => Some(cs.get(Colors::StopTurn)),
                }
            }
        }
    }

    pub fn show_turn_icons(&self, id: IntersectionID) -> bool {
        match self {
            StopSignEditor::Active(i) => *i == id,
            StopSignEditor::Inactive => false,
        }
    }
}
