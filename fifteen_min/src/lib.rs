use widgetry::Settings;

#[macro_use]
extern crate log;

mod find_home;
mod isochrone;
mod viewer;

type App = map_gui::SimpleApp<()>;

pub fn main() {
    let settings = Settings::new("15-minute neighborhoods").read_svg(Box::new(abstio::slurp_bytes));
    run(settings);
}

fn run(settings: Settings) {
    widgetry::run(settings, |ctx| {
        map_gui::SimpleApp::new(ctx, map_gui::options::Options::default(), (), |ctx, app| {
            vec![viewer::Viewer::random_start(ctx, app)]
        })
    });
}

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = "run")]
pub fn run_wasm(root_dom_id: String, assets_base_url: String) {
    // We haven't set up logging yet, so this logging is dropped. We can't setup logging just yet,
    // since logging is also enabled as part of `SimpleApp::new`.
    // TODO: Should we make log set-up explicit rather than a side effect of CmdArgs::new?
    // CmdArgs::new();
    log::info!(
        "starting with root_dom_id: {}, assets_base_url: {}",
        root_dom_id,
        assets_base_url
    );

    let settings = Settings::new("15-minute neighborhoods")
        .root_dom_element_id(root_dom_id)
        .assets_base_url(assets_base_url)
        .read_svg(Box::new(abstio::slurp_bytes));

    run(settings);
}
