//! The map_editor renders and lets you edit RawMaps, which are a format in between OSM and the
//! full Map. It's useful for debugging maps imported from OSM, and for drawing synthetic maps for
//! testing.

#[macro_use]
extern crate log;

use widgetry::Settings;

use crate::app::App;

mod app;
mod edit;
mod load;
mod model;
mod world;

pub fn main() {
    let settings = Settings::new("RawMap editor");
    run(settings);
}

fn run(mut settings: Settings) {
    settings = settings.read_svg(Box::new(abstio::slurp_bytes));
    widgetry::run(settings, |ctx| {
        let mut args = abstutil::CmdArgs::new();
        let load = args.optional_free();
        let include_bldgs = args.enabled("--bldgs");
        let center_camera = args.optional("--cam");
        args.done();

        let mut app = App {
            model: model::Model::blank(ctx),
        };
        app.model.include_bldgs = include_bldgs;

        let states = if let Some(path) = load {
            // In case the initial load fails, stick a blank state at the bottom
            vec![
                app::MainState::new_state(ctx, &app),
                load::load_map(ctx, path, include_bldgs, center_camera),
            ]
        } else {
            vec![app::MainState::new_state(ctx, &app)]
        };
        (app, states)
    });
}

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = "run")]
pub fn run_wasm(root_dom_id: String, assets_base_url: String, assets_are_gzipped: bool) {
    let settings = Settings::new("RawMap editor")
        .root_dom_element_id(root_dom_id)
        .assets_base_url(assets_base_url)
        .assets_are_gzipped(assets_are_gzipped);

    run(settings);
}
