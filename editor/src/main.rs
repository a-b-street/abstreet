mod abtest;
mod colors;
mod common;
mod debug;
mod edit;
mod game;
mod mission;
mod objects;
mod plugins;
mod render;
mod sandbox;
mod state;
mod tutorial;
mod ui;

use structopt::StructOpt;

fn main() {
    ezgui::run("A/B Street", 1024.0, 768.0, |canvas, prerender| {
        game::GameState::new(state::Flags::from_args(), canvas, prerender)
    });
}
