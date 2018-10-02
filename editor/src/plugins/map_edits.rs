use control::ControlMap;
use ezgui::{Canvas, GfxCtx, UserInput};
use map_model::Map;
use plugins::Colorizer;

// TODO ahh, something needs to remember edits_name.

pub struct EditsManager {}

impl EditsManager {
    pub fn new() -> EditsManager {
        EditsManager {}
    }

    pub fn event(&mut self, input: &mut UserInput, map: &Map, control_map: &ControlMap) -> bool {
        false
    }

    pub fn draw(&self, g: &mut GfxCtx, canvas: &Canvas) {}
}

impl Colorizer for EditsManager {}
