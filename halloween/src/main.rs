mod render;
mod timer;

use crate::render::DrawMap;
use crate::timer::Cycler;
use abstutil::Timer;
use ezgui::{Canvas, EventLoopMode, GfxCtx, Key, Prerender, UserInput, GUI};
use map_model::{Map, MapEdits};
use std::process;
use structopt::StructOpt;

#[derive(StructOpt)]
#[structopt(name = "halloween")]
struct Flags {
    /// Map to render
    #[structopt(name = "load_map")]
    load_map: String,
}

const ANIMATION_PERIOD_S: f64 = 2.0;

struct UI {
    canvas: Canvas,
    draw_map: DrawMap,
    cycler: Cycler,
}

impl UI {
    fn new(flags: Flags, canvas: Canvas) -> UI {
        let map = Map::new(
            &flags.load_map,
            MapEdits::new("map name"),
            &mut Timer::new("load map for Halloween"),
        )
        .unwrap();
        UI {
            canvas,
            draw_map: DrawMap::new(map),
            cycler: Cycler::new(ANIMATION_PERIOD_S),
        }
    }
}

impl GUI<()> for UI {
    fn event(&mut self, input: &mut UserInput, _: &Prerender) -> (EventLoopMode, ()) {
        if input.unimportant_key_pressed(Key::Escape, "quit") {
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
    ezgui::run("Halloween tech demo", 1024.0, 768.0, |canvas, _| {
        UI::new(flags, canvas)
    });
}
