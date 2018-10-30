use ezgui::{GfxCtx, Text};
use objects::{Ctx, ID};
use piston::input::Key;
use plugins::{Plugin, PluginCtx};

pub enum DebugObjectsState {
    Empty,
    Selected(ID),
    Tooltip(ID),
}

impl DebugObjectsState {
    pub fn new() -> DebugObjectsState {
        DebugObjectsState::Empty
    }
}

impl Plugin for DebugObjectsState {
    fn event(&mut self, ctx: PluginCtx) -> bool {
        let new_state = if let Some(id) = ctx.primary.current_selection {
            // Don't break out of the tooltip state
            if let DebugObjectsState::Tooltip(_) = self {
                DebugObjectsState::Tooltip(id)
            } else {
                DebugObjectsState::Selected(id)
            }
        } else {
            DebugObjectsState::Empty
        };
        *self = new_state;

        let mut new_state: Option<DebugObjectsState> = None;
        match self {
            DebugObjectsState::Empty => {}
            DebugObjectsState::Selected(id) => {
                if ctx.input.key_pressed(Key::LCtrl, &format!("Hold Ctrl to show {:?}'s tooltip", id)) {
                    new_state = Some(DebugObjectsState::Tooltip(*id));
                } else if ctx.input.key_pressed(Key::D, "debug") {
                    id.debug(&ctx.primary.map, &ctx.primary.control_map, &mut ctx.primary.sim, &ctx.primary.draw_map);
                }
            }
            DebugObjectsState::Tooltip(id) => {
                if ctx.input.key_released(Key::LCtrl) {
                    new_state = Some(DebugObjectsState::Selected(*id));
                }
            }
        };
        if let Some(s) = new_state {
            *self = s;
        }
        match self {
            DebugObjectsState::Empty => false,
            // TODO hmm, but when we press D to debug, we don't want other stuff to happen...
            DebugObjectsState::Selected(_) => false,
            DebugObjectsState::Tooltip(_) => true,
        }
    }

    fn draw(&self, g: &mut GfxCtx, ctx: Ctx) {
        match *self {
            DebugObjectsState::Empty => {}
            DebugObjectsState::Selected(_) => {}
            DebugObjectsState::Tooltip(id) => {
                let mut txt = Text::new();
                for line in id.tooltip_lines(ctx.map, ctx.draw_map, ctx.sim) {
                    txt.add_line(line);
                }
                ctx.canvas.draw_mouse_tooltip(g, txt);
            }
        }
    }
}
