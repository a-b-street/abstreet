mod abtest;
mod common;
mod debug;
mod edit;
mod game;
mod helpers;
mod mission;
mod render;
mod sandbox;
mod splash_screen;
mod state;
mod tutorial;
mod ui;

use structopt::StructOpt;

fn main() {
    ezgui::run("A/B Street", 1800.0, 800.0, |ctx| {
        game::Game::new(ui::Flags::from_args(), ctx)
    });
}
