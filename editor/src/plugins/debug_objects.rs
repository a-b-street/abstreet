use control::ControlMap;
use ezgui::{Canvas, GfxCtx, UserInput};
use map_model::Map;
use objects::ID;
use piston::input::{Button, Key, ReleaseEvent};
use render::{DrawMap, Renderable};
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
                    debug(id, map, control_map, sim);
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
                canvas.draw_mouse_tooltip(g, &tooltip_lines(id, map, draw_map, sim));
            }
        }
    }
}

// TODO move to Renderable or ID?
fn debug(id: &ID, map: &Map, control_map: &ControlMap, sim: &mut Sim) {
    match id {
        ID::Lane(id) => {
            map.get_l(*id).dump_debug();
        }
        ID::Intersection(id) => {
            map.get_i(*id).dump_debug();
            sim.debug_intersection(*id, control_map);
        }
        ID::Turn(_) => {}
        ID::Building(id) => {
            map.get_b(*id).dump_debug();
        }
        ID::Car(_) => {}
        ID::Pedestrian(id) => {
            sim.debug_ped(*id);
        }
        ID::ExtraShape(_) => {}
    }
}

fn tooltip_lines(id: ID, map: &Map, draw_map: &DrawMap, sim: &Sim) -> Vec<String> {
    match id {
        ID::Lane(id) => draw_map.get_l(id).tooltip_lines(map),
        ID::Building(id) => draw_map.get_b(id).tooltip_lines(map),
        ID::Car(id) => sim.car_tooltip(id),
        ID::Pedestrian(id) => sim.ped_tooltip(id),
        ID::Intersection(id) => vec![format!("{}", id)],
        ID::Turn(id) => map.get_t(id).tooltip_lines(map),
        ID::ExtraShape(id) => draw_map.get_es(id).tooltip_lines(map),
    }
}
