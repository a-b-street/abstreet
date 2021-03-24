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
#[wasm_bindgen(js_name = "runWithRootId")]
pub fn run_in_dom_element(root_dom_id: String) {
    // currently this logging is dropped because logging is set up as part of `SimpleApp::new`
    // should we make log set-up explicit rather than a side effect of CmdArgs::new?
    log::info!("starting with root_dom_id: {}", root_dom_id);

    let settings = Settings::new("15-minute neighborhoods")
        .root_dom_element_id(&root_dom_id)
        .read_svg(Box::new(abstio::slurp_bytes));

    run(settings);
}
