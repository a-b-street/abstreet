#![allow(clippy::type_complexity)]

use widgetry::Settings;

#[macro_use]
extern crate log;

mod viewer;

type App = map_gui::SimpleApp<()>;

pub fn main() {
    let settings = Settings::new("Low traffic neighborhoods");
    run(settings);
}

fn run(mut settings: Settings) {
    let options = map_gui::options::Options::load_or_default();
    settings = settings
        .read_svg(Box::new(abstio::slurp_bytes))
        .canvas_settings(options.canvas_settings.clone());
    widgetry::run(settings, |ctx| {
        map_gui::SimpleApp::new(ctx, options, (), |ctx, app| {
            vec![
                map_gui::tools::TitleScreen::new_state(
                    ctx,
                    app,
                    map_gui::tools::Executable::LTN,
                    Box::new(|ctx, app, _| viewer::Viewer::new_state(ctx, app)),
                ),
                viewer::Viewer::new_state(ctx, app),
            ]
        })
    });
}

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = "run")]
pub fn run_wasm(root_dom_id: String, assets_base_url: String, assets_are_gzipped: bool) {
    let settings = Settings::new("Low traffic neighborhoods")
        .root_dom_element_id(root_dom_id)
        .assets_base_url(assets_base_url)
        .assets_are_gzipped(assets_are_gzipped);

    run(settings);
}
