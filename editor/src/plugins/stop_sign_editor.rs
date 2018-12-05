// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use ezgui::Color;
use map_model::{IntersectionID, TurnPriority};
use objects::{Ctx, ID};
use piston::input::Key;
use plugins::{Plugin, PluginCtx};

#[derive(PartialEq)]
pub enum StopSignEditor {
    Inactive,
    Active(IntersectionID),
}

impl StopSignEditor {
    pub fn new() -> StopSignEditor {
        StopSignEditor::Inactive
    }

    pub fn show_turn_icons(&self, id: IntersectionID) -> bool {
        match self {
            StopSignEditor::Active(i) => *i == id,
            StopSignEditor::Inactive => false,
        }
    }
}

impl Plugin for StopSignEditor {
    fn event(&mut self, ctx: PluginCtx) -> bool {
        let input = ctx.input;
        let map = &mut ctx.primary.map;
        let selected = ctx.primary.current_selection;

        if *self == StopSignEditor::Inactive {
            match selected {
                Some(ID::Intersection(id)) => {
                    if map.maybe_get_stop_sign(id).is_some()
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
                    if id.parent == *i {
                        let mut sign = map.get_stop_sign(*i).clone();

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
                            _ => {}
                        };

                        map.edit_stop_sign(sign);
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

    fn color_for(&self, obj: ID, ctx: Ctx) -> Option<Color> {
        match (self, obj) {
            (StopSignEditor::Active(i), ID::Turn(t)) => {
                if t.parent != *i {
                    return Some(ctx.cs.get("irrelevant turn", Color::grey(0.3)));
                }
                match ctx.map.get_stop_sign(*i).get_priority(t) {
                    TurnPriority::Priority => {
                        Some(ctx.cs.get("priority stop sign turn", Color::GREEN))
                    }
                    TurnPriority::Yield => Some(ctx.cs.get("yield stop sign turn", Color::YELLOW)),
                    TurnPriority::Stop => Some(ctx.cs.get("stop turn", Color::RED)),
                    TurnPriority::Banned => Some(ctx.cs.get("banned turn", Color::PURPLE)),
                }
            }
            _ => None,
        }
    }
}
