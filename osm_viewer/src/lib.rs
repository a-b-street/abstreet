mod viewer;

pub fn main() {
    widgetry::run(widgetry::Settings::new("OpenStreetMap viewer"), |ctx| {
        let app = map_gui::SimpleApp::new(ctx, abstutil::CmdArgs::new(), ());
        let states = vec![viewer::Viewer::new(ctx, &app)];
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
