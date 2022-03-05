#[macro_use]
extern crate log;

use structopt::StructOpt;

mod mapper;

fn main() {
    let mut options = map_gui::options::Options::load_or_default();
    options.canvas_settings.min_zoom_for_detail = 2.0;
    let args = map_gui::SimpleAppArgs::from_iter(abstutil::cli_args());
    args.override_options(&mut options);

    let settings = args
        .update_widgetry_settings(widgetry::Settings::new("OSM parking mapper"))
        .canvas_settings(options.canvas_settings.clone());
    widgetry::run(settings, |ctx| {
        map_gui::SimpleApp::new(ctx, options, args.map_name(), args.cam, (), |ctx, app| {
            vec![
                map_gui::tools::TitleScreen::new_state(
                    ctx,
                    app,
                    map_gui::tools::Executable::ParkingMapper,
                    Box::new(|ctx, app, _| mapper::ParkingMapper::new_state(ctx, app)),
                ),
                mapper::ParkingMapper::new_state(ctx, app),
            ]
        })
    });
}
