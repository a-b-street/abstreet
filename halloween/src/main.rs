mod render;
mod timer;

use crate::render::DrawMap;
use crate::timer::Cycler;
use abstutil::Timer;
use ezgui::{Canvas, EventLoopMode, GfxCtx, UserInput, GUI};
use map_model::{Map, MapEdits};
use piston::input::Key;
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
const ANIMATION_PERIOD_S: f64 = 2.0;

struct UI {
    canvas: Canvas,
    draw_map: DrawMap,
    cycler: Cycler,
}

impl UI {
    fn new(flags: Flags) -> UI {
        let map = Map::new(
            &flags.load_map,
            MapEdits::new("map name"),
            &mut Timer::new("load map for Halloween"),
        )
        .unwrap();
        UI {
            canvas: Canvas::new(),
            draw_map: DrawMap::new(map),
            cycler: Cycler::new(ANIMATION_PERIOD_S),
        }
    }
}

impl GUI<()> for UI {
    fn event(&mut self, input: &mut UserInput) -> (EventLoopMode, ()) {
        if input.unimportant_key_pressed(Key::Escape, KEY_CATEGORY, "quit") {
            process::exit(0);
        }
        self.canvas.handle_event(input);
        (EventLoopMode::Animation, ())
    }

    fn get_mut_canvas(&mut self) -> &mut Canvas {
        &mut self.canvas
    }

    fn draw(&self, g: &mut GfxCtx, _: &()) {
        self.draw_map
            .draw(g, self.cycler.value(), self.canvas.get_screen_bounds());
    }
}

fn main() {
    let flags = Flags::from_args();
    ezgui::run(UI::new(flags), "Halloween tech demo", 1024, 768);
}
