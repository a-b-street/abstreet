use ezgui::{Canvas, GfxCtx, Text};
use map_model::Map;
use objects::ID;
use piston::input::Key;
use plugins::{Colorizer, PluginCtx};
use render::DrawMap;
use sim::Sim;

pub enum DebugObjectsState {
    Empty,
    Selected(ID),
    Tooltip(ID),
}

impl DebugObjectsState {
    pub fn new() -> DebugObjectsState {
        DebugObjectsState::Empty
    }

    pub fn draw(&self, map: &Map, canvas: &Canvas, draw_map: &DrawMap, sim: &Sim, g: &mut GfxCtx) {
        match *self {
            DebugObjectsState::Empty => {}
            DebugObjectsState::Selected(_) => {}
            DebugObjectsState::Tooltip(id) => {
                let mut txt = Text::new();
                for line in id.tooltip_lines(map, draw_map, sim) {
                    txt.add_line(line);
                }
                canvas.draw_mouse_tooltip(g, txt);
            }
        }
    }
}

impl Colorizer for DebugObjectsState {
    fn event(&mut self, ctx: PluginCtx) -> bool {
        let (selected, input, map, sim, control_map) = (
            ctx.primary.current_selection,
            ctx.input,
            &ctx.primary.map,
            &mut ctx.primary.sim,
            &ctx.primary.control_map,
        );

        let new_state = if let Some(id) = selected {
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
                if input.key_pressed(Key::LCtrl, &format!("Hold Ctrl to show {:?}'s tooltip", id)) {
                    new_state = Some(DebugObjectsState::Tooltip(*id));
                } else if input.key_pressed(Key::D, "debug") {
                    id.debug(map, control_map, sim);
                }
            }
            DebugObjectsState::Tooltip(id) => {
                if input.key_released(Key::LCtrl) {
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
}
