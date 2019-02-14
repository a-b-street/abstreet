#[macro_use]
mod macros;

mod colors;
mod objects;
mod plugins;
mod render;
mod state;
mod tutorial;
mod ui;

use structopt::StructOpt;

fn main() {
    let flags = state::Flags::from_args();

    if flags.sim_flags.load == "../data/raw_maps/ban_left_turn.abst" {
        ezgui::run("A/B Street", 1024.0, 768.0, |mut canvas, prerender| {
            ui::UI::new(
                tutorial::TutorialState::new(flags, &mut canvas, prerender),
                canvas,
            )
        });
    } else {
        ezgui::run("A/B Street", 1024.0, 768.0, |canvas, prerender| {
            ui::UI::new(
                state::DefaultUIState::new(flags, &canvas, prerender, true),
                canvas,
            )
        });
    }
}
