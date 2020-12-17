#[macro_use]
extern crate log;

mod after_level;
mod animation;
mod before_level;
mod buildings;
mod controls;
mod game;
mod levels;
mod meters;
mod music;
mod player;
mod session;
mod title;
mod vehicles;

type App = map_gui::SimpleApp<session::Session>;
type Transition = widgetry::Transition<App>;

pub fn main() {
    widgetry::run(widgetry::Settings::new("experiment"), |ctx| {
        let mut opts = map_gui::options::Options::default();
        opts.color_scheme = map_gui::colors::ColorSchemeChoice::NightMode;
        let mut app = map_gui::SimpleApp::new_with_opts(
            ctx,
            abstutil::CmdArgs::new(),
            opts,
            session::Session::load(),
        );
        if app.opts.dev {
            app.session.unlock_all();
        }
        app.session.music = music::Music::start(ctx, app.session.play_music);
        app.session.music.specify_volume(music::OUT_OF_GAME);

        let states = vec![title::TitleScreen::new(ctx, &app)];
        (app, states)
    });
}

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn run() {
    main();
}
