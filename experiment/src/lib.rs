#[macro_use]
extern crate log;

mod animation;
mod buildings;
mod controls;
mod game;
mod levels;
mod meters;
mod movement;
mod upzone;
mod vehicles;

pub fn main() {
    widgetry::run(widgetry::Settings::new("experiment"), |ctx| {
        let mut opts = map_gui::options::Options::default();
        opts.color_scheme = map_gui::colors::ColorSchemeChoice::NightMode;
        let app = map_gui::SimpleApp::new_with_opts(ctx, abstutil::CmdArgs::new(), opts);
        let states = vec![levels::TitleScreen::new(ctx)];
        (app, states)
    });
}

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn run() {
    main();
}
