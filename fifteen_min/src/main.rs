mod isochrone;
mod viewer;

fn main() {
    widgetry::run(widgetry::Settings::new("15-minute neighborhoods"), |ctx| {
        let app = map_gui::SimpleApp::new(ctx, abstutil::CmdArgs::new());
        let states = vec![viewer::Viewer::random_start(ctx, &app)];
        (app, states)
    });
}
