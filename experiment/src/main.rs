mod animation;
mod controls;
mod game;
mod upzone;

fn main() {
    widgetry::run(widgetry::Settings::new("experiment"), |ctx| {
        let app = map_gui::SimpleApp::new(ctx, abstutil::CmdArgs::new());
        let states = ctx.loading_screen("setup", |ctx, mut timer| {
            vec![game::Game::new(ctx, &app, &mut timer)]
        });
        (app, states)
    });
}
