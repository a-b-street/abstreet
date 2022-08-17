#![allow(clippy::type_complexity)]

use structopt::StructOpt;

use abstio::MapName;
use abstutil::Timer;
use map_gui::tools::DrawSimpleRoadLabels;
use map_model::{AmenityType, RoutingParams};
use widgetry::{Color, Drawable, EventCtx, GeomBatch, GfxCtx, RewriteColor, Settings};

pub use browse::BrowseNeighbourhoods;
use filters::Toggle3Zoomed;
pub use filters::{DiagonalFilter, Edits, FilterType};
pub use neighbourhood::{Cell, DistanceInterval, Neighbourhood};
pub use partition::{NeighbourhoodID, Partitioning};

#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate log;

mod browse;
mod colors;
mod components;
mod connectivity;
mod customize_boundary;
mod draw_cells;
mod edit;
mod export;
mod filters;
mod impact;
mod neighbourhood;
mod partition;
mod route_planner;
mod save;
mod select_boundary;
mod shortcuts;

type App = map_gui::SimpleApp<Session>;
type Transition = widgetry::Transition<App>;

pub fn main() {
    let settings = Settings::new("Low traffic neighbourhoods");
    run(settings);
}

#[derive(StructOpt)]
struct Args {
    /// Load a previously saved proposal with this name. Note this takes a name, not a full path.
    #[structopt(long)]
    proposal: Option<String>,
    /// Lock the user into one fixed neighbourhood, and remove many controls
    #[structopt(long)]
    consultation: Option<String>,
    #[structopt(flatten)]
    app_args: map_gui::SimpleAppArgs,
}

fn run(mut settings: Settings) {
    let mut opts = map_gui::options::Options::load_or_default();
    opts.color_scheme = map_gui::colors::ColorSchemeChoice::ClassicLTN;
    opts.show_building_driveways = false;
    // TODO Ideally we would have a better map model in the first place. The next best thing would
    // be to change these settings based on the map's country, but that's a bit tricky to do early
    // enough (before map_switched). So for now, assume primary use of this tool is in the UK,
    // where these settings are most appropriate.
    opts.show_stop_signs = false;
    opts.show_crosswalks = false;
    opts.show_traffic_signal_icon = true;
    opts.simplify_basemap = true;
    opts.canvas_settings.min_zoom_for_detail = std::f64::MAX;

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
            edits: Edits::default(),
            routing_params_before_changes: RoutingParams::default(),
            draw_all_road_labels: None,
            draw_poi_icons: Drawable::empty(ctx),

            alt_proposals: save::AltProposals::new(),
            draw_all_filters: Toggle3Zoomed::empty(ctx),
            impact: impact::Impact::empty(ctx),

            edit_mode: edit::EditMode::Filters,
            filter_type: FilterType::WalkCycleOnly,

            draw_neighbourhood_style: browse::Style::Simple,
            draw_cells_as_areas: true,
            heuristic: filters::auto::Heuristic::SplitCells,
            main_road_penalty: 1.0,

            current_trip_name: None,

            consultation: None,
            consultation_proposal_path: None,
        };
        map_gui::SimpleApp::new(
            ctx,
            opts,
            args.app_args.map_name(),
            args.app_args.cam,
            session,
            move |ctx, app| {
                // Restore the partitioning from a file before calling BrowseNeighbourhoods
                let popup_state = args.proposal.as_ref().and_then(|name| {
                    crate::save::Proposal::load(
                        ctx,
                        app,
                        abstio::path_ltn_proposals(app.map.get_name(), name),
                    )
                });

                let mut states = Vec::new();
                if let Some(ref consultation) = args.consultation {
                    if app.map.get_name() != &MapName::new("gb", "bristol", "east") {
                        panic!("Consultation mode not supported on this map");
                    }

                    let focus_on_street = match consultation.as_ref() {
                        "pt1" => "Gregory Street",
                        "pt2" => {
                            // Start from a baked-in proposal with special boundaries
                            app.session.consultation_proposal_path = Some(abstio::path(
                                "system/ltn_proposals/bristol_beaufort_road.json.gz",
                            ));
                            "Jubilee Road"
                        }
                        _ => panic!("Unknown Bristol consultation mode {consultation}"),
                    };

                    app.session.alt_proposals = crate::save::AltProposals::new();
                    ctx.loading_screen("initialize", |ctx, timer| {
                        crate::clear_current_proposal(ctx, app, timer);
                    });

                    // Look for the neighbourhood containing one small street
                    let r = app
                        .map
                        .all_roads()
                        .iter()
                        .find(|r| r.get_name(None) == focus_on_street)
                        .expect(&format!("Can't find {focus_on_street}"))
                        .id;
                    let (neighbourhood, _) = app
                        .session
                        .partitioning
                        .all_neighbourhoods()
                        .iter()
                        .find(|(_, info)| info.block.perimeter.interior.contains(&r))
                        .expect(&format!(
                            "Can't find neighbourhood containing {focus_on_street}"
                        ));
                    app.session.consultation = Some(*neighbourhood);

                    // TODO Maybe center the camera, ignoring any saved values

                    states.push(connectivity::Viewer::new_state(
                        ctx,
                        app,
                        app.session.consultation.unwrap(),
                    ));
                } else {
                    states.push(map_gui::tools::TitleScreen::new_state(
                        ctx,
                        app,
                        map_gui::tools::Executable::LTN,
                        Box::new(|ctx, app, _| BrowseNeighbourhoods::new_state(ctx, app)),
                    ));
                    states.push(BrowseNeighbourhoods::new_state(ctx, app));
                }
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
    let settings = Settings::new("Low traffic neighbourhoods")
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
    pub edits: Edits,
    // These capture modal filters that exist in the map already. Whenever we pathfind in this app
    // in the "before changes" case, we have to use these. Do NOT use the map's built-in
    // pathfinder. (https://github.com/a-b-street/abstreet/issues/852 would make this more clear)
    pub routing_params_before_changes: RoutingParams,
    pub draw_all_road_labels: Option<DrawSimpleRoadLabels>,
    pub draw_poi_icons: Drawable,

    pub alt_proposals: save::AltProposals,
    pub draw_all_filters: Toggle3Zoomed,
    pub impact: impact::Impact,

    pub edit_mode: edit::EditMode,
    pub filter_type: FilterType,

    // Remember form settings in different tabs.
    // Browse neighbourhoods:
    pub draw_neighbourhood_style: browse::Style,
    // Connectivity:
    pub draw_cells_as_areas: bool,
    pub heuristic: filters::auto::Heuristic,
    // Pathfinding
    pub main_road_penalty: f64,

    current_trip_name: Option<String>,

    consultation: Option<NeighbourhoodID>,
    // The current consultation should always be based off a built-in proposal
    consultation_proposal_path: Option<String>,
}

/// Do the equivalent of `SimpleApp::draw_unzoomed`, but after the water/park areas layer, draw
/// something custom.
fn draw_with_layering<F: Fn(&mut GfxCtx)>(g: &mut GfxCtx, app: &App, custom: F) {
    g.clear(app.cs.void_background);
    g.redraw(&app.draw_map.boundary_polygon);
    g.redraw(&app.draw_map.draw_all_areas);
    custom(g);
    g.redraw(&app.draw_map.draw_all_unzoomed_parking_lots);
    g.redraw(&app.draw_map.draw_all_unzoomed_roads_and_intersections);
    g.redraw(&app.draw_map.draw_all_buildings);
    g.redraw(&app.draw_map.draw_all_building_outlines);
}

pub fn after_edit(ctx: &EventCtx, app: &mut App) {
    app.session.draw_all_filters = app.session.edits.draw(ctx, &app.map);
}

pub fn clear_current_proposal(ctx: &mut EventCtx, app: &mut App, timer: &mut Timer) {
    if let Some(path) = app.session.consultation_proposal_path.clone() {
        if crate::save::Proposal::load(ctx, app, path.clone()).is_some() {
            panic!("Consultation mode broken; go fix {path} manually");
        }
        return;
    }

    app.session.proposal_name = None;
    // Reset this first. transform_existing_filters will fill some out.
    app.session.routing_params_before_changes = RoutingParams::default();
    app.session.edits = Edits::default();
    crate::filters::transform_existing_filters(ctx, app, timer);
    app.session.partitioning = Partitioning::seed_using_heuristics(app, timer);
    app.session.draw_all_filters = app.session.edits.draw(ctx, &app.map);
    app.session.draw_all_road_labels = None;
    app.session.draw_poi_icons = render_poi_icons(ctx, app);
}

fn render_poi_icons(ctx: &EventCtx, app: &App) -> Drawable {
    let mut batch = GeomBatch::new();
    let school = GeomBatch::load_svg(ctx, "system/assets/map/school.svg")
        .scale(0.2)
        .color(RewriteColor::ChangeAll(Color::WHITE));

    for b in app.map.all_buildings() {
        if b.amenities.iter().any(|a| {
            let at = AmenityType::categorize(&a.amenity_type);
            at == Some(AmenityType::School) || at == Some(AmenityType::University)
        }) {
            batch.append(school.clone().centered_on(b.polygon.polylabel()));
        }
    }

    ctx.upload(batch)
}
