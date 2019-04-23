mod colors;
mod edit;
mod game;
mod objects;
mod plugins;
mod render;
mod state;
mod tutorial;
mod ui;

use structopt::StructOpt;

fn main() {
    ezgui::run("A/B Street", 1024.0, 768.0, |canvas, prerender| {
        game::GameState::new(state::Flags::from_args(), canvas, prerender)
    });
}
