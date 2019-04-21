mod colors;
mod objects;
mod plugins;
mod render;
mod splash;
mod state;
mod tutorial;
mod ui;

use std::path::Path;
use structopt::StructOpt;

fn main() {
    let flags = state::Flags::from_args();

    if flags.splash {
        ezgui::run("A/B Street", 1024.0, 768.0, |canvas, prerender| {
            splash::GameState::new(flags, canvas, prerender)
        });
    } else if flags.sim_flags.load == Path::new("../data/maps/ban_left_turn.abst") {
        ezgui::run("A/B Street", 1024.0, 768.0, |canvas, prerender| {
            ui::UI::new(tutorial::TutorialState::new(flags, prerender), canvas)
        });
    } else {
        ezgui::run("A/B Street", 1024.0, 768.0, |canvas, prerender| {
            ui::UI::new(state::DefaultUIState::new(flags, prerender, true), canvas)
        });
    }
}
