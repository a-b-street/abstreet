#[macro_use]
extern crate log;

mod mapper;

fn main() {
    widgetry::run(
        widgetry::Settings::new("OSM parking mapper").read_svg(Box::new(abstio::slurp_bytes)),
        |ctx| {
            let mut opts = map_gui::options::Options::default();
            opts.min_zoom_for_detail = 2.0;
            map_gui::SimpleApp::new(ctx, opts, (), |ctx, app| {
                vec![mapper::ParkingMapper::new(ctx, app)]
            })
        },
    );
}
