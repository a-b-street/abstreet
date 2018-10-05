// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

extern crate aabb_quadtree;
extern crate abstutil;
extern crate control;
extern crate dimensioned;
extern crate ezgui;
extern crate flame;
#[macro_use]
extern crate generator;
extern crate geo;
extern crate geom;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;
extern crate map_model;
extern crate piston;
extern crate quick_xml;
#[macro_use]
extern crate pretty_assertions;
extern crate rand;
#[macro_use]
extern crate serde_derive;
extern crate sim;
#[macro_use]
extern crate structopt;
extern crate strum;
#[macro_use]
extern crate strum_macros;
extern crate yansi;

#[macro_use]
mod macros;

mod colors;
mod kml;
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

    /// Extra KML to display
    #[structopt(long = "kml")]
    kml: Option<String>,
}

fn main() {
    let flags = Flags::from_args();
    ezgui::run(
        ui::UIWrapper::new(flags.sim_flags, flags.kml),
        "A/B Street",
        1024,
        768,
    );
}
