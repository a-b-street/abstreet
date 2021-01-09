mod viewer;

pub fn main() {
    widgetry::run(
        widgetry::Settings::new("OpenStreetMap viewer").read_svg(Box::new(abstio::slurp_bytes)),
        |ctx| {
            map_gui::SimpleApp::new(ctx, map_gui::options::Options::default(), (), |ctx, app| {
                vec![viewer::Viewer::new(ctx, app)]
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
