#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate log;

use widgetry::Settings;

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
    let settings = Settings::new("15-minute Santa");
    run(settings);
}

fn run(mut settings: Settings) {
    let mut opts = map_gui::options::Options::load_or_default();
    opts.color_scheme = map_gui::colors::ColorSchemeChoice::NightMode;
    // Note we don't take any CLI arguments at all. Always start on the first level's map

    settings = settings
        .read_svg(Box::new(abstio::slurp_bytes))
        .window_icon(abstio::path("system/assets/pregame/icon.png"))
        .canvas_settings(opts.canvas_settings.clone());
    widgetry::run(settings, |ctx| {
        let session = session::Session::load();
        session.save();

        // On native, we may not have this file. Start with a blank map if so. When we try to pick
        // a level on the title screen, we'll download it if needed.
        let mut start_map = Some(abstio::MapName::seattle("qa"));
        if cfg!(not(target_arch = "wasm32"))
            && !abstio::file_exists(start_map.as_ref().unwrap().path())
        {
            start_map = None;
        }

        map_gui::SimpleApp::new(ctx, opts, start_map, None, session, |ctx, app| {
            if app.opts.dev {
                app.session.unlock_all();
            }
            app.session.music = music::Music::start(ctx, app.session.play_music, "jingle_bells");
            app.session.music.specify_volume(music::OUT_OF_GAME);

            vec![title::TitleScreen::new_state(ctx, app)]
        })
    });
}

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = "run")]
pub fn run_wasm(root_dom_id: String, assets_base_url: String, assets_are_gzipped: bool) {
    let settings = Settings::new("15-minute Santa")
        .root_dom_element_id(root_dom_id)
        .assets_base_url(assets_base_url)
        .assets_are_gzipped(assets_are_gzipped);

    run(settings);
}
