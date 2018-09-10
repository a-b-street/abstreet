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
extern crate graphics;
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

mod colors;
mod kml;
mod plugins;
mod render;
mod ui;

use structopt::StructOpt;

#[derive(StructOpt, Debug)]
#[structopt(name = "editor")]
struct Flags {
    /// Map or savestate to load
    #[structopt(name = "load")]
    load: String,

    /// Optional RNG seed
    #[structopt(long = "rng_seed")]
    rng_seed: Option<u8>,

    /// Extra KML to display
    #[structopt(long = "kml")]
    kml: Option<String>,

    /// Scenario name for savestating
    #[structopt(long = "scenario_name", default_value = "editor")]
    scenario_name: String,
}

fn main() {
    let flags = Flags::from_args();
    ezgui::run(
        ui::UI::new(flags.load, flags.scenario_name, flags.rng_seed, flags.kml),
        "A/B Street",
        1024,
        768,
    );
}
