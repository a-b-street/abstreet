#![allow(clippy::type_complexity)]

use structopt::StructOpt;

use abstutil::Timer;
use widgetry::{EventCtx, GfxCtx, Settings};

pub use browse::BrowseNeighborhoods;
use filters::Toggle3Zoomed;
pub use filters::{DiagonalFilter, ModalFilters};
pub use neighborhood::{Cell, DistanceInterval, Neighborhood};
pub use partition::{NeighborhoodID, Partitioning};

#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate log;

mod browse;
mod colors;
mod common;
mod connectivity;
mod draw_cells;
mod export;
mod filters;
mod impact;
mod neighborhood;
mod partition;
mod per_neighborhood;
mod rat_run_viewer;
mod rat_runs;
mod route_planner;
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
    opts.color_scheme = map_gui::colors::ColorSchemeChoice::LTN;
    let args = Args::from_iter(abstutil::cli_args());
    args.app_args.override_options(&mut opts);

    settings = args
        .app_args
        .update_widgetry_settings(settings)
        .canvas_settings(opts.canvas_settings.clone());
    widgetry::run(settings, move |ctx| {
        let session = Session {
            proposal_name: None,
            partitioning: Partitioning::empty(),
            modal_filters: ModalFilters::default(),

            alt_proposals: save::AltProposals::new(),
            draw_all_filters: Toggle3Zoomed::empty(ctx),
            impact: impact::Impact::empty(ctx),

            highlight_boundary_roads: false,
            draw_neighborhood_style: browse::Style::SimpleColoring,
            draw_cells_as_areas: true,
            heuristic: filters::auto::Heuristic::OnlyOneBorder,
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
                let popup_state = args
                    .proposal
                    .as_ref()
                    .and_then(|name| crate::save::Proposal::load(ctx, app, name));

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

// TODO Tension: Many of these are per-map. game::App nicely wraps these up. Time to stop abusing
// SimpleApp?
pub struct Session {
    // These come from a save::Proposal
    pub proposal_name: Option<String>,
    pub partitioning: Partitioning,
    pub modal_filters: ModalFilters,

    pub alt_proposals: save::AltProposals,
    pub draw_all_filters: Toggle3Zoomed,
    pub impact: impact::Impact,

    // Remember form settings in different tabs.
    // Browse neighborhoods:
    pub highlight_boundary_roads: bool,
    pub draw_neighborhood_style: browse::Style,
    // Connectivity:
    pub draw_cells_as_areas: bool,
    pub heuristic: filters::auto::Heuristic,
    // Pathfinding
    pub main_road_penalty: f64,

    current_trip_name: Option<String>,
}

/// Do the equivalent of `SimpleApp::draw_unzoomed` or `draw_zoomed`, but after the water/park
/// areas layer, draw something custom.
fn draw_with_layering<F: Fn(&mut GfxCtx)>(g: &mut GfxCtx, app: &App, custom: F) {
    g.clear(app.cs.void_background);
    g.redraw(&app.draw_map.boundary_polygon);
    g.redraw(&app.draw_map.draw_all_areas);
    custom(g);

    if g.canvas.is_unzoomed() {
        g.redraw(&app.draw_map.draw_all_unzoomed_parking_lots);
        g.redraw(&app.draw_map.draw_all_unzoomed_roads_and_intersections);
        g.redraw(&app.draw_map.draw_all_buildings);
        g.redraw(&app.draw_map.draw_all_building_outlines);
    } else {
        let options = map_gui::render::DrawOptions::new();
        let objects = app
            .draw_map
            .get_renderables_back_to_front(g.get_screen_bounds(), &app.map);

        let mut drawn_all_buildings = false;

        for obj in objects {
            obj.draw(g, app, &options);

            if matches!(obj.get_id(), map_gui::ID::Building(_)) && !drawn_all_buildings {
                g.redraw(&app.draw_map.draw_all_buildings);
                g.redraw(&app.draw_map.draw_all_building_outlines);
                drawn_all_buildings = true;
            }
        }
    }
}

pub fn after_edit(ctx: &EventCtx, app: &mut App) {
    app.session.draw_all_filters = app.session.modal_filters.draw(ctx, &app.map);
}

pub fn clear_current_proposal(ctx: &EventCtx, app: &mut App, timer: &mut Timer) {
    app.session.proposal_name = None;
    // Reset this first. transform_existing_filters will fill some out.
    app.session.modal_filters = ModalFilters::default();
    crate::filters::transform_existing_filters(ctx, app, timer);
    app.session.partitioning = Partitioning::seed_using_heuristics(app, timer);
    app.session.draw_all_filters = app.session.modal_filters.draw(ctx, &app.map);
}
