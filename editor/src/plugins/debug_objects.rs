use control::ControlMap;
use ezgui::{Canvas, GfxCtx, UserInput};
use map_model::Map;
use objects::ID;
use piston::input::{Button, Key, ReleaseEvent};
use plugins::Colorizer;
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

    pub fn event(
        &mut self,
        selected: Option<ID>,
        input: &mut UserInput,
        map: &Map,
        sim: &mut Sim,
        control_map: &ControlMap,
    ) -> bool {
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
                if let Some(Button::Keyboard(Key::LCtrl)) =
                    input.use_event_directly().release_args()
                {
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

    pub fn draw(&self, map: &Map, canvas: &Canvas, draw_map: &DrawMap, sim: &Sim, g: &mut GfxCtx) {
        match *self {
            DebugObjectsState::Empty => {}
            DebugObjectsState::Selected(_) => {}
            DebugObjectsState::Tooltip(id) => {
                canvas.draw_mouse_tooltip(g, &id.tooltip_lines(map, draw_map, sim));
            }
        }
    }
}

impl Colorizer for DebugObjectsState {}
