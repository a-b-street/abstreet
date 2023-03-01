#![allow(clippy::type_complexity)]

use structopt::StructOpt;

use widgetry::Settings;

#[macro_use]
extern crate log;

mod amenities_details;
mod bus;
mod common;
mod find_amenities;
mod find_home;
mod isochrone;
mod render;
mod single_start;

type App = map_gui::SimpleApp<()>;

pub fn main() {
    let settings = Settings::new("15-minute neighborhoods");
    run(settings);
}

fn run(mut settings: Settings) {
    let mut options = map_gui::options::Options::load_or_default();
    let args = map_gui::SimpleAppArgs::from_iter(abstutil::cli_args());
    args.override_options(&mut options);

    settings = args
        .update_widgetry_settings(settings)
        .canvas_settings(options.canvas_settings.clone());
    widgetry::run(settings, |ctx| {
        map_gui::SimpleApp::new(
            ctx,
            options,
            Some(args.map_name()),
            args.cam,
            (),
            |ctx, app| vec![single_start::SingleStart::random_start(ctx, app)],
        )
    });
}

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = "run")]
pub fn run_wasm(root_dom_id: String, assets_base_url: String, assets_are_gzipped: bool) {
    let settings = Settings::new("15-minute neighborhoods")
        .root_dom_element_id(root_dom_id)
        .assets_base_url(assets_base_url)
        .assets_are_gzipped(assets_are_gzipped);

    run(settings);
}
