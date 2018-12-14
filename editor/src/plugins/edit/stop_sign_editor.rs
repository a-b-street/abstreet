use crate::objects::{Ctx, ID};
use crate::plugins::{Plugin, PluginCtx};
use crate::render::{draw_stop_sign, stop_sign_rendering_hints};
use ezgui::{Color, GfxCtx};
use map_model::{ControlStopSign, IntersectionID, TurnPriority};
use piston::input::Key;

pub struct StopSignEditor {
    i: IntersectionID,
}

impl StopSignEditor {
    pub fn new(ctx: &mut PluginCtx) -> Option<StopSignEditor> {
        if let Some(ID::Intersection(id)) = ctx.primary.current_selection {
            if ctx.primary.map.maybe_get_stop_sign(id).is_some()
                && ctx
                    .input
                    .contextual_action(Key::E, &format!("edit stop signs for {}", id))
            {
                return Some(StopSignEditor { i: id });
            }
        }
        None
    }

    pub fn show_turn_icons(&self, id: IntersectionID) -> bool {
        self.i == id
    }
}

impl Plugin for StopSignEditor {
    fn blocking_event(&mut self, ctx: &mut PluginCtx) -> bool {
        let input = &mut ctx.input;
        let map = &mut ctx.primary.map;
        let selected = ctx.primary.current_selection;

        stop_sign_rendering_hints(&mut ctx.hints, map.get_stop_sign(self.i), map, ctx.cs);

        if let Some(ID::Turn(id)) = selected {
            let mut sign = map.get_stop_sign(self.i).clone();
            let next_priority = match sign.get_priority(id) {
                TurnPriority::Banned => TurnPriority::Stop,
                TurnPriority::Stop => TurnPriority::Yield,
                TurnPriority::Yield => {
                    if sign.could_be_priority_turn(id, map) {
                        TurnPriority::Priority
                    } else {
                        TurnPriority::Banned
                    }
                }
                TurnPriority::Priority => TurnPriority::Banned,
            };
            if input.contextual_action(Key::Space, &format!("toggle to {:?}", next_priority)) {
                sign.set_priority(id, next_priority, map);
                map.edit_stop_sign(sign);
            }
        } else if input.key_pressed(Key::Return, "quit the editor") {
            return false;
        } else if input.key_pressed(Key::R, "reset to default stop sign") {
            let sign = ControlStopSign::new(map, self.i);
            map.edit_stop_sign(sign);
        }
        true
    }

    fn draw(&self, g: &mut GfxCtx, ctx: &Ctx) {
        draw_stop_sign(ctx.map.get_stop_sign(self.i), g, ctx.cs, ctx.map);
    }

    fn color_for(&self, obj: ID, ctx: &Ctx) -> Option<Color> {
        if let ID::Turn(t) = obj {
            if t.parent != self.i {
                return None;
            }
            match ctx.map.get_stop_sign(self.i).get_priority(t) {
                TurnPriority::Priority => {
                    Some(ctx.cs.get_def("priority stop sign turn", Color::GREEN))
                }
                TurnPriority::Yield => Some(ctx.cs.get_def("yield stop sign turn", Color::YELLOW)),
                TurnPriority::Stop => Some(ctx.cs.get_def("stop turn", Color::RED)),
                TurnPriority::Banned => Some(ctx.cs.get_def("banned turn", Color::BLACK)),
            }
        } else {
            None
        }
    }
}
