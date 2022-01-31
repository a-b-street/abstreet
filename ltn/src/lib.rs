#![allow(clippy::type_complexity)]

use structopt::StructOpt;

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
mod impact;
mod neighborhood;
mod partition;
mod pathfinding;
mod per_neighborhood;
mod rat_run_viewer;
mod rat_runs;
mod save;
mod select_boundary;

type App = map_gui::SimpleApp<Session>;
type Transition = widgetry::Transition<App>;

pub fn main() {
    let settings = Settings::new("Low traffic neighborhoods");
    run(settings);
}

#[derive(StructOpt)]
struct Args {
    /// Load a previously saved proposal with this name. Note this takes a name, not a full path.
    #[structopt(long)]
    proposal: Option<String>,
    #[structopt(flatten)]
    app_args: map_gui::SimpleAppArgs,
}

fn run(mut settings: Settings) {
    let mut opts = map_gui::options::Options::load_or_default();
    opts.color_scheme = map_gui::colors::ColorSchemeChoice::DayMode;
    let args = Args::from_iter(abstutil::cli_args());
    args.app_args.override_options(&mut opts);

    settings = settings
        .read_svg(Box::new(abstio::slurp_bytes))
        .canvas_settings(opts.canvas_settings.clone());
    widgetry::run(settings, move |ctx| {
        let session = Session {
            partitioning: Partitioning::empty(),
            modal_filters: ModalFilters::default(),

            impact: impact::Impact::empty(ctx),

            highlight_boundary_roads: true,
            draw_neighborhood_style: browse::Style::SimpleColoring,
            draw_cells_as_areas: true,
            draw_borders_as_arrows: true,
            heuristic: auto::Heuristic::OnlyOneBorder,
            main_road_penalty: 1.0,

            current_trip_name: None,
        };
        map_gui::SimpleApp::new(
            ctx,
            opts,
            args.app_args.map_name(),
            args.app_args.cam,
            session,
            move |ctx, app| {
                // Restore the partitioning from a file before calling BrowseNeighborhoods
                let popup_state = if let Some(name) = &args.proposal {
                    ctx.loading_screen("load existing proposal", |ctx, mut timer| {
                        match crate::save::Proposal::load(app, name, &mut timer) {
                            Ok(()) => None,
                            Err(err) => Some(map_gui::tools::PopupMsg::new_state(
                                ctx,
                                "Error",
                                vec![format!("Couldn't load proposal {}", name), err.to_string()],
                            )),
                        }
                    })
                } else {
                    None
                };

                let mut states = vec![
                    map_gui::tools::TitleScreen::new_state(
                        ctx,
                        app,
                        map_gui::tools::Executable::LTN,
                        Box::new(|ctx, app, _| BrowseNeighborhoods::new_state(ctx, app)),
                    ),
                    BrowseNeighborhoods::new_state(ctx, app),
                ];
                if let Some(state) = popup_state {
                    states.push(state);
                }
                states
            },
        )
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

    pub impact: impact::Impact,

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
