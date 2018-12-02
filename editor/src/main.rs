// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

extern crate aabb_quadtree;
extern crate abstutil;
extern crate counter;
extern crate cpuprofiler;
extern crate dimensioned;
#[macro_use]
extern crate downcast;
extern crate ezgui;
#[macro_use]
extern crate generator;
extern crate geo;
extern crate geom;
extern crate kml;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;
extern crate map_model;
extern crate nbez;
extern crate piston;
extern crate quick_xml;
#[macro_use]
extern crate pretty_assertions;
extern crate rand;
#[macro_use]
extern crate serde_derive;
extern crate sim;
extern crate structopt;

#[macro_use]
mod macros;

mod colors;
mod objects;
mod plugins;
mod render;
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
    ezgui::run(
        ui::UI::new(flags.sim_flags, flags.kml),
        "A/B Street",
        1024,
        768,
    );
}
