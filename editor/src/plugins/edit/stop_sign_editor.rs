use crate::objects::{DrawCtx, ID};
use crate::plugins::{BlockingPlugin, PluginCtx};
use abstutil::Timer;
use ezgui::{Color, Key};
use map_model::{ControlStopSign, IntersectionID, TurnPriority};

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

impl BlockingPlugin for StopSignEditor {
    fn blocking_event(&mut self, ctx: &mut PluginCtx) -> bool {
        let input = &mut ctx.input;
        let map = &mut ctx.primary.map;
        let selected = ctx.primary.current_selection;

        input.set_mode_with_prompt(
            "Stop Sign Editor",
            format!("Stop Sign Editor for {}", self.i),
            &ctx.canvas,
        );

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
        } else if input.modal_action("quit") {
            return false;
        } else if input.modal_action("reset to default") {
            let sign = ControlStopSign::new(map, self.i, &mut Timer::new("reset ControlStopSign"));
            map.edit_stop_sign(sign);
        }
        true
    }

    fn color_for(&self, obj: ID, ctx: &DrawCtx) -> Option<Color> {
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
