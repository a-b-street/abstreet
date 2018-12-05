use ezgui::{Color, GfxCtx};
use map_model::{ControlStopSign, IntersectionID, TurnPriority};
use objects::{Ctx, ID};
use piston::input::Key;
use plugins::{Plugin, PluginCtx};
use render::{draw_stop_sign, stop_sign_rendering_hints};

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
    fn event(&mut self, mut ctx: PluginCtx) -> bool {
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
                stop_sign_rendering_hints(&mut ctx.hints, map.get_stop_sign(*i), map, ctx.cs);

                if let Some(ID::Turn(id)) = selected {
                    let mut sign = map.get_stop_sign(*i).clone();
                    let next_priority = match sign.get_priority(id) {
                        TurnPriority::Banned => TurnPriority::Stop,
                        TurnPriority::Stop => TurnPriority::Yield,
                        TurnPriority::Yield => if sign.could_be_priority_turn(id, map) {
                            TurnPriority::Priority
                        } else {
                            TurnPriority::Banned
                        },
                        TurnPriority::Priority => TurnPriority::Banned,
                    };
                    if input.key_pressed(Key::Space, &format!("toggle to {:?}", next_priority)) {
                        sign.set_priority(id, next_priority, map);
                        map.edit_stop_sign(sign);
                    }
                } else if input.key_pressed(Key::Return, "quit the editor") {
                    new_state = Some(StopSignEditor::Inactive);
                } else if input.key_pressed(Key::R, "reset to default stop sign") {
                    let sign = ControlStopSign::new(map, *i);
                    map.edit_stop_sign(sign);
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

    fn draw(&self, g: &mut GfxCtx, ctx: Ctx) {
        match self {
            StopSignEditor::Inactive => {}
            StopSignEditor::Active(id) => {
                draw_stop_sign(ctx.map.get_stop_sign(*id), g, ctx.cs, ctx.map);
            }
        }
    }

    fn color_for(&self, obj: ID, ctx: Ctx) -> Option<Color> {
        match (self, obj) {
            (StopSignEditor::Active(i), ID::Turn(t)) => {
                if t.parent != *i {
                    return None;
                }
                match ctx.map.get_stop_sign(*i).get_priority(t) {
                    TurnPriority::Priority => {
                        Some(ctx.cs.get("priority stop sign turn", Color::GREEN))
                    }
                    TurnPriority::Yield => Some(ctx.cs.get("yield stop sign turn", Color::YELLOW)),
                    TurnPriority::Stop => Some(ctx.cs.get("stop turn", Color::RED)),
                    TurnPriority::Banned => Some(ctx.cs.get("banned turn", Color::BLACK)),
                }
            }
            _ => None,
        }
    }
}
