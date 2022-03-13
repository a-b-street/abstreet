//! The map_editor renders and lets you edit RawMaps, which are a format in between OSM and the
//! full Map. It's useful for debugging maps imported from OSM, and for drawing synthetic maps for
//! testing.

#[macro_use]
extern crate log;

use structopt::StructOpt;

use widgetry::Settings;

use crate::app::App;

mod app;
mod camera;
mod edit;
mod load;
mod model;

pub fn main() {
    let settings = Settings::new("RawMap editor");
    run(settings);
}

#[derive(StructOpt)]
#[structopt(name = "run_scenario", about = "Simulates a scenario")]
struct Args {
    /// The path to a RawMap to load. If omitted, start with a blank map.
    #[structopt()]
    load: Option<String>,
    /// Import buildings from the RawMap. Slow.
    #[structopt(long)]
    include_buildings: bool,
    /// The initial camera state
    #[structopt(long)]
    cam: Option<String>,
}

fn run(mut settings: Settings) {
    abstutil::logger::setup();

    settings = settings
        .read_svg(Box::new(abstio::slurp_bytes))
        .window_icon(abstio::path("system/assets/pregame/icon.png"));
    widgetry::run(settings, |ctx| {
        let args = Args::from_iter(abstutil::cli_args());
        let mut app = App {
            model: model::Model::blank(ctx),
        };
        app.model.include_bldgs = args.include_buildings;

        let states = if let Some(path) = args.load {
            // In case the initial load fails, stick a blank state at the bottom
            vec![
                app::MainState::new_state(ctx, &app),
                load::load_map(ctx, path, args.include_buildings, args.cam),
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
