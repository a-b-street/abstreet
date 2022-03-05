mod viewer;

use structopt::StructOpt;

use widgetry::Settings;

pub fn main() {
    let settings = Settings::new("OpenStreetMap viewer");
    run(settings)
}

pub fn run(mut settings: Settings) {
    let mut opts = map_gui::options::Options::load_or_default();
    opts.show_building_driveways = false;
    let args = map_gui::SimpleAppArgs::from_iter(abstutil::cli_args());
    args.override_options(&mut opts);

    settings = args
        .update_widgetry_settings(settings)
        .canvas_settings(opts.canvas_settings.clone());
    widgetry::run(settings, |ctx| {
        map_gui::SimpleApp::new(ctx, opts, args.map_name(), args.cam, (), |ctx, app| {
            vec![
                map_gui::tools::TitleScreen::new_state(
                    ctx,
                    app,
                    map_gui::tools::Executable::OSMViewer,
                    Box::new(|ctx, app, _| viewer::Viewer::new_state(ctx, app)),
                ),
                viewer::Viewer::new_state(ctx, app),
            ]
        })
    });
}

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = "run")]
pub fn run_wasm(root_dom_id: String, assets_base_url: String, assets_are_gzipped: bool) {
    let settings = Settings::new("OpenStreetMap viewer")
        .root_dom_element_id(root_dom_id)
        .assets_base_url(assets_base_url)
        .assets_are_gzipped(assets_are_gzipped);

    run(settings);
}
