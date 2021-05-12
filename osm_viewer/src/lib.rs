#[macro_use]
extern crate log;

mod viewer;

use widgetry::Settings;

pub fn main() {
    let settings = Settings::new("OpenStreetMap viewer").read_svg(Box::new(abstio::slurp_bytes));
    run(settings)
}

pub fn run(mut settings: Settings) {
    settings = settings.read_svg(Box::new(abstio::slurp_bytes));

    widgetry::run(settings, |ctx| {
        map_gui::SimpleApp::new(ctx, map_gui::options::Options::default(), (), |ctx, app| {
            vec![viewer::Viewer::new_state(ctx, app)]
        })
    });
}

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = "run")]
pub fn run_wasm(root_dom_id: String, assets_base_url: String, assets_are_gzipped: bool) {
    let settings = Settings::new("OpenStreetMap viewer")
        .root_dom_element_id(root_dom_id)
        .assets_base_url(assets_base_url)
        .assets_are_gzipped(assets_are_gzipped);

    run(settings);
}
