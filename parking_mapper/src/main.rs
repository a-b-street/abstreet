mod mapper;

fn main() {
    widgetry::run(
        widgetry::Settings::new("OSM parking mapper").read_svg(Box::new(abstio::slurp_bytes)),
        |ctx| {
            let mut app = map_gui::SimpleApp::new(ctx, abstutil::CmdArgs::new(), ());
            app.opts.min_zoom_for_detail = 2.0;
            let states = vec![mapper::ParkingMapper::new(ctx, &app)];
            (app, states)
        },
    );
}
