// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

#[macro_use]
mod macros;

mod colors;
mod init_colors;
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
    let canvas = ezgui::Canvas::new();
    if flags.sim_flags.load == "../data/raw_maps/ban_left_turn.abst".to_string() {
        ezgui::run(
            ui::UI::new(
                tutorial::TutorialState::new(flags.sim_flags, &canvas),
                canvas,
            ),
            "A/B Street",
            1024,
            768,
        );
    } else {
        ezgui::run(
            ui::UI::new(
                state::DefaultUIState::new(flags.sim_flags, flags.kml, &canvas),
                canvas,
            ),
            "A/B Street",
            1024,
            768,
        );
    }
}
