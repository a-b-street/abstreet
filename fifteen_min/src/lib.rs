#[macro_use]
extern crate log;

mod find_home;
mod isochrone;
mod viewer;

type App = map_gui::SimpleApp<()>;

pub fn main() {
    widgetry::run(
        widgetry::Settings::new("15-minute neighborhoods").read_svg(Box::new(abstio::slurp_bytes)),
        |ctx| {
            map_gui::SimpleApp::new(ctx, map_gui::options::Options::default(), (), |ctx, app| {
                vec![viewer::Viewer::random_start(ctx, app)]
            })
        },
    );
}

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn run() {
    main();
}
