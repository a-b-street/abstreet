#[macro_use]
mod macros;

mod colors;
mod objects;
mod plugins;
mod render;
mod state;
mod tutorial;
mod ui;

use sim::SimFlags;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
#[structopt(name = "editor")]
struct Flags {
    #[structopt(flatten)]
    sim_flags: SimFlags,

    /// Extra KML or ExtraShapes to display
    #[structopt(long = "kml")]
    kml: Option<String>,
}

fn main() {
    let flags = Flags::from_args();
    /*cpuprofiler::PROFILER
    .lock()
    .unwrap()
    .start("./profile")
    .unwrap();*/

    let mut canvas = ezgui::Canvas::new(1024, 768);
    let cs = colors::ColorScheme::load().unwrap();

    if flags.sim_flags.load == "../data/raw_maps/ban_left_turn.abst" {
        ezgui::run(
            ui::UI::new(
                tutorial::TutorialState::new(flags.sim_flags, &mut canvas, &cs),
                canvas,
                cs,
            ),
            "A/B Street",
        );
    } else {
        ezgui::run(
            ui::UI::new(
                state::DefaultUIState::new(flags.sim_flags, flags.kml, &canvas, &cs, true),
                canvas,
                cs,
            ),
            "A/B Street",
        );
    }
}
