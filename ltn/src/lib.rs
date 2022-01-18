#![allow(clippy::type_complexity)]

use widgetry::Settings;

pub use browse::BrowseNeighborhoods;
pub use filters::{DiagonalFilter, ModalFilters};
pub use neighborhood::{Cell, DistanceInterval, Neighborhood};
pub use partition::{NeighborhoodID, Partitioning};

#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate log;

mod auto;
mod browse;
mod connectivity;
mod draw_cells;
mod export;
mod filters;
mod neighborhood;
mod partition;
mod pathfinding;
mod per_neighborhood;
mod rat_run_viewer;
mod rat_runs;
mod select_boundary;

type App = map_gui::SimpleApp<Session>;
type Transition = widgetry::Transition<App>;

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
        let session = Session {
            partitioning: Partitioning::empty(),
            modal_filters: ModalFilters::default(),
        };
        map_gui::SimpleApp::new(ctx, options, session, |ctx, app| {
            vec![
                map_gui::tools::TitleScreen::new_state(
                    ctx,
                    app,
                    map_gui::tools::Executable::LTN,
                    Box::new(|ctx, app, _| BrowseNeighborhoods::new_state(ctx, app)),
                ),
                BrowseNeighborhoods::new_state(ctx, app),
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

pub struct Session {
    pub partitioning: Partitioning,
    pub modal_filters: ModalFilters,
}
