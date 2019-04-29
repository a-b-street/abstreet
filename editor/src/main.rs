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
mod tutorial;
mod ui;

use structopt::StructOpt;

fn main() {
    ezgui::run("A/B Street", 1024.0, 768.0, |canvas, prerender| {
        game::GameState::new(ui::Flags::from_args(), canvas, prerender)
    });
}
