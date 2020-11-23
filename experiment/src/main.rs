mod game;

fn main() {
    widgetry::run(widgetry::Settings::new("experiment"), |ctx| {
        ctx.canvas.cam_zoom = 10.0; // TODO
        let app = map_gui::SimpleApp::new(ctx, abstutil::CmdArgs::new());
        (app, vec![game::Game::new(ctx)])
    });
}
