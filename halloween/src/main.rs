mod render;
mod timer;

use crate::render::DrawMap;
use crate::timer::Cycler;
use abstutil::Timer;
use ezgui::{EventCtx, EventLoopMode, GfxCtx, Key, GUI};
use map_model::Map;
use std::path::{Path, PathBuf};
use std::process;
use structopt::StructOpt;

#[derive(StructOpt)]
#[structopt(name = "halloween")]
struct Flags {
    /// Map to render
    #[structopt(name = "load_map", parse(from_os_str))]
    load_map: PathBuf,
}

const ANIMATION_PERIOD_S: f64 = 2.0;

struct UI {
    draw_map: DrawMap,
    cycler: Cycler,
}

impl UI {
    fn new(flags: Flags, ctx: &mut EventCtx) -> UI {
        // TODO Consolidate with sim::load
        let map: Map = if flags.load_map.starts_with(Path::new("../data/raw_maps/")) {
            Map::new(
                flags.load_map.to_str().unwrap(),
                &mut Timer::new("load map"),
            )
            .unwrap()
        } else {
            abstutil::read_binary(
                flags.load_map.to_str().unwrap(),
                &mut Timer::new("load map"),
            )
            .unwrap()
        };

        UI {
            draw_map: DrawMap::new(map, ctx.prerender),
            cycler: Cycler::new(ANIMATION_PERIOD_S),
        }
    }
}

impl GUI for UI {
    fn event(&mut self, ctx: &mut EventCtx) -> EventLoopMode {
        if ctx.input.unimportant_key_pressed(Key::Escape, "quit") {
            process::exit(0);
        }
        ctx.canvas.handle_event(ctx.input);
        EventLoopMode::Animation
    }

    fn draw(&self, g: &mut GfxCtx) {
        self.draw_map.draw(g, self.cycler.value());
    }
}

fn main() {
    let flags = Flags::from_args();
    ezgui::run("Halloween tech demo", 1024.0, 768.0, |ctx| {
        UI::new(flags, ctx)
    });
}
