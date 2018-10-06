extern crate ezgui;
extern crate geom;
extern crate map_model;
extern crate piston;
#[macro_use]
extern crate structopt;

mod render;

use ezgui::{Canvas, EventLoopMode, GfxCtx, Text, UserInput, GUI};
use map_model::{Map, RoadEdits};
use piston::input::Key;
use render::DrawMap;
use std::process;
use structopt::StructOpt;

#[derive(StructOpt)]
#[structopt(name = "halloween")]
struct Flags {
    /// Map to render
    #[structopt(name = "load_map")]
    load_map: String,
}

const KEY_CATEGORY: &str = "";

struct UI {
    canvas: Canvas,
    draw_map: DrawMap,
}

impl UI {
    fn new(flags: Flags) -> UI {
        let map = Map::new(&flags.load_map, RoadEdits::new()).unwrap();
        UI {
            canvas: Canvas::new(),
            draw_map: DrawMap::new(map),
        }
    }
}

impl GUI for UI {
    fn event(&mut self, mut input: UserInput, _osd: &mut Text) -> EventLoopMode {
        if input.unimportant_key_pressed(Key::Escape, KEY_CATEGORY, "quit") {
            process::exit(0);
        }

        self.canvas.handle_event(&mut input);

        EventLoopMode::InputOnly
    }

    fn get_mut_canvas(&mut self) -> &mut Canvas {
        &mut self.canvas
    }

    fn draw(&self, g: &mut GfxCtx, _osd: Text) {
        self.draw_map.draw(g);
    }
}

fn main() {
    let flags = Flags::from_args();
    ezgui::run(UI::new(flags), "Halloween tech demo", 1024, 768);
}
