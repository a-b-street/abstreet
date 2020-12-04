mod viewer;

fn main() {
    widgetry::run(widgetry::Settings::new("OpenStreetMap viewer"), |ctx| {
        let app = map_gui::SimpleApp::new(ctx, abstutil::CmdArgs::new(), ());
        let states = vec![viewer::Viewer::new(ctx, &app)];
        (app, states)
    });
}
