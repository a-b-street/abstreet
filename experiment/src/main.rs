mod animation;
mod controls;
mod game;
mod levels;
mod upzone;

fn main() {
    widgetry::run(widgetry::Settings::new("experiment"), |ctx| {
        let mut opts = map_gui::options::Options::default();
        opts.color_scheme = map_gui::colors::ColorSchemeChoice::NightMode;
        let app = map_gui::SimpleApp::new_with_opts(ctx, abstutil::CmdArgs::new(), opts);
        let states = vec![levels::TitleScreen::new(ctx)];
        (app, states)
    });
}
