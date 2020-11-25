mod controls;
mod game;

fn main() {
    widgetry::run(widgetry::Settings::new("experiment"), |ctx| {
        let app = map_gui::SimpleApp::new(ctx, abstutil::CmdArgs::new());
        let states = vec![game::Game::new(ctx, &app)];
        (app, states)
    });
}
