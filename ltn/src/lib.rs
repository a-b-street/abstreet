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
    let mut opts = map_gui::options::Options::load_or_default();
    opts.color_scheme = map_gui::colors::ColorSchemeChoice::DayMode;
    settings = settings
        .read_svg(Box::new(abstio::slurp_bytes))
        .canvas_settings(opts.canvas_settings.clone());
    widgetry::run(settings, |ctx| {
        let session = Session {
            partitioning: Partitioning::empty(),
            modal_filters: ModalFilters::default(),

            highlight_boundary_roads: true,
            draw_neighborhood_style: browse::Style::SimpleColoring,
            draw_cells_as_areas: true,
            draw_borders_as_arrows: true,
            heuristic: auto::Heuristic::OnlyOneBorder,
            main_road_penalty: 1.0,

            current_trip_name: None,
        };
        map_gui::SimpleApp::new(ctx, opts, session, |ctx, app| {
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

    // Remember form settings in different tabs.
    // Browse neighborhoods:
    pub highlight_boundary_roads: bool,
    pub draw_neighborhood_style: browse::Style,
    // Connectivity:
    pub draw_cells_as_areas: bool,
    pub draw_borders_as_arrows: bool,
    pub heuristic: auto::Heuristic,
    // Pathfinding
    pub main_road_penalty: f64,

    current_trip_name: Option<String>,
}
