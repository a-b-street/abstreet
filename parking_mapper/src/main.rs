#[macro_use]
extern crate log;

mod mapper;

fn main() {
    let mut options = map_gui::options::Options::load_or_default();
    options.min_zoom_for_detail = 2.0;
    let settings = widgetry::Settings::new("OSM parking mapper")
        .read_svg(Box::new(abstio::slurp_bytes))
        .canvas_settings(options.canvas_settings.clone());
    widgetry::run(settings, |ctx| {
        map_gui::SimpleApp::new(ctx, options, (), |ctx, app| {
            vec![mapper::ParkingMapper::new_state(ctx, app)]
        })
    });
}
