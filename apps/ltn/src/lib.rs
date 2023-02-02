#![allow(clippy::type_complexity)]

use structopt::StructOpt;

use abstio::MapName;
use map_model::{Map, PathConstraints, Road};
use widgetry::tools::FutureLoader;
use widgetry::{EventCtx, Settings, State};

pub use app::{App, PerMap, Session, Transition};
use filters::Toggle3Zoomed;
pub use filters::{Crossing, DiagonalFilter, Edits, FilterType, RoadFilter};
pub use neighbourhood::{Cell, DistanceInterval, Neighbourhood};
pub use partition::{NeighbourhoodID, Partitioning};
pub use pick_area::PickArea;

#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate log;

mod app;
mod colors;
mod components;
mod crossings;
mod customize_boundary;
mod design_ltn;
mod draw_cells;
mod edit;
mod export;
mod filters;
mod freehand_boundary;
mod impact;
mod neighbourhood;
mod partition;
mod per_resident_impact;
mod pick_area;
mod route_planner;
mod save;
mod select_boundary;
mod shortcuts;

pub fn main() {
    let settings = Settings::new("Low traffic neighbourhoods");
    run(settings);
}

#[derive(StructOpt)]
struct Args {
    /// Load a previously saved proposal with this name. Note this takes a name, not a full path.
    /// Or `remote/<ID>`.
    #[structopt(long)]
    proposal: Option<String>,
    /// Lock the user into one fixed neighbourhood, and remove many controls
    #[structopt(long)]
    consultation: Option<String>,
    #[structopt(flatten)]
    app_args: map_gui::SimpleAppArgs,
}

const SPRITE_WIDTH: u32 = 750;
const SPRITE_HEIGHT: u32 = 458;

fn run(mut settings: Settings) {
    let mut opts = map_gui::options::Options::load_or_default();
    opts.color_scheme = map_gui::colors::ColorSchemeChoice::LTN;
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

    settings = settings.load_default_textures(false);
    settings = args
        .app_args
        .update_widgetry_settings(settings)
        .canvas_settings(opts.canvas_settings.clone());
    widgetry::run(settings, move |ctx| {
        // This file is small enough to bundle in the build
        ctx.set_texture(
            include_bytes!("../spritesheet.gif").to_vec(),
            (SPRITE_WIDTH, SPRITE_HEIGHT),
            (SPRITE_WIDTH as f32, SPRITE_HEIGHT as f32),
        );

        App::new(
            ctx,
            opts,
            args.app_args.map_name(),
            args.app_args.cam,
            move |ctx, app| {
                // We need app to fully initialize this
                app.session
                    .layers
                    .event(ctx, &app.cs, components::Mode::PickArea, None);

                if let Some(ref name) = args.proposal {
                    // Remote edits require another intermediate state to load
                    if let Some(id) = name.strip_prefix("remote/") {
                        vec![load_remote(ctx, id.to_string(), args.consultation.clone())]
                    } else {
                        let popup_state = crate::save::Proposal::load_from_path(
                            ctx,
                            app,
                            abstio::path_ltn_proposals(app.per_map.map.get_name(), name),
                        );
                        setup_initial_states(ctx, app, args.consultation.as_ref(), popup_state)
                    }
                } else {
                    setup_initial_states(ctx, app, args.consultation.as_ref(), None)
                }
            },
        )
    });
}

// A Proposal should already be loaded by now, unless consultation is set
fn setup_initial_states(
    ctx: &mut EventCtx,
    app: &mut App,
    consultation: Option<&String>,
    popup_state: Option<Box<dyn State<App>>>,
) -> Vec<Box<dyn State<App>>> {
    let mut states = Vec::new();
    if let Some(ref consultation) = consultation {
        if app.per_map.map.get_name() != &MapName::new("gb", "bristol", "east") {
            panic!("Consultation mode not supported on this map");
        }

        let mut consultation_proposal_path = None;

        let focus_on_street = match consultation.as_ref() {
            "pt1" => "Gregory Street",
            "pt2" => {
                // Start from a baked-in proposal with special boundaries
                consultation_proposal_path = Some(abstio::path(
                    "system/ltn_proposals/bristol_beaufort_road.json.gz",
                ));
                "Jubilee Road"
            }
            _ => panic!("Unknown Bristol consultation mode {consultation}"),
        };

        // If we already loaded something from a saved proposal, then don't clear anything
        if let Some(path) = consultation_proposal_path {
            if crate::save::Proposal::load_from_path(ctx, app, path.clone()).is_some() {
                panic!("Consultation mode broken; go fix {path} manually");
            }
            app.per_map.proposals.clear_all_but_current();
            // TODO Kind of a weird hack -- rename this to "existing LTNs" so we can't overwrite
            // it!
            app.per_map.proposals.current_proposal.name = "existing LTNs".to_string();
        }

        // Look for the neighbourhood containing one small street
        let r = app
            .per_map
            .map
            .all_roads()
            .iter()
            .find(|r| r.get_name(None) == focus_on_street)
            .expect(&format!("Can't find {focus_on_street}"))
            .id;
        let (neighbourhood, _) = app
            .partitioning()
            .all_neighbourhoods()
            .iter()
            .find(|(_, info)| info.block.perimeter.interior.contains(&r))
            .expect(&format!(
                "Can't find neighbourhood containing {focus_on_street}"
            ));
        app.per_map.consultation = Some(*neighbourhood);
        app.per_map.consultation_id = Some(consultation.to_string());

        // TODO Maybe center the camera, ignoring any saved values

        states.push(design_ltn::DesignLTN::new_state(
            ctx,
            app,
            app.per_map.consultation.unwrap(),
        ));
    } else {
        states.push(PickArea::new_state(ctx, app));
    }
    if let Some(state) = popup_state {
        states.push(state);
    }
    states
}

fn load_remote(
    ctx: &mut EventCtx,
    id: String,
    consultation: Option<String>,
) -> Box<dyn State<App>> {
    let (_, outer_progress_rx) = futures_channel::mpsc::channel(1);
    let (_, inner_progress_rx) = futures_channel::mpsc::channel(1);
    let url = format!("{}/get-ltn?id={}", crate::save::PROPOSAL_HOST_URL, id);
    FutureLoader::<App, Vec<u8>>::new_state(
        ctx,
        Box::pin(async move {
            let bytes = abstio::http_get(url).await?;
            let wrapper: Box<dyn Send + FnOnce(&App) -> Vec<u8>> = Box::new(move |_| bytes);
            Ok(wrapper)
        }),
        outer_progress_rx,
        inner_progress_rx,
        "Downloading proposal",
        Box::new(move |ctx, app, result| {
            let popup_state = crate::save::Proposal::load_from_bytes(ctx, app, &id, result);
            Transition::Clear(setup_initial_states(
                ctx,
                app,
                consultation.as_ref(),
                popup_state,
            ))
        }),
    )
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

pub fn redraw_all_filters(ctx: &EventCtx, app: &mut App) {
    app.per_map.draw_all_filters = app.edits().draw(ctx, &app.per_map.map);
}

fn is_private(road: &Road) -> bool {
    // See https://wiki.openstreetmap.org/wiki/Tag:access%3Dprivate#Relation_to_access=no
    road.osm_tags.is_any("access", vec!["no", "private"])
}

fn is_driveable(road: &Road, map: &Map) -> bool {
    PathConstraints::Car.can_use_road(road, map) && !is_private(road)
}

// The current edits and partitioning are stored deeply nested in App. For read-only access, we can
// use a regular helper method. For writing, we can't, because we'll get a borrow error -- so
// instead just use macros to make it less annoying to modify
#[macro_export]
macro_rules! mut_edits {
    ($app:ident) => {
        $app.per_map.proposals.current_proposal.edits
    };
}

#[macro_export]
macro_rules! mut_partitioning {
    ($app:ident) => {
        $app.per_map.proposals.current_proposal.partitioning
    };
}
