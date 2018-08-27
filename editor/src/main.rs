// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

extern crate aabb_quadtree;
extern crate abstutil;
extern crate control;
extern crate dimensioned;
extern crate ezgui;
#[macro_use]
extern crate generator;
extern crate geo;
extern crate geom;
extern crate glutin_window;
extern crate graphics;
extern crate map_model;
extern crate opengl_graphics;
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
mod experimental;
mod gui;
mod kml;
mod plugins;
mod render;
mod ui;

use ezgui::input::UserInput;
use glutin_window::GlutinWindow;
use opengl_graphics::{Filter, GlGraphics, GlyphCache, OpenGL, TextureSettings};
use piston::event_loop::{EventLoop, EventSettings, Events};
use piston::input::RenderEvent;
use piston::window::{Window, WindowSettings};
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
#[structopt(name = "editor")]
struct Flags {
    /// ABST input to load
    #[structopt(name = "abst_input")]
    abst_input: String,

    /// Optional RNG seed
    #[structopt(long = "rng_seed")]
    rng_seed: Option<u8>,

    /// Use the experimental GUI
    #[structopt(long = "experimental")]
    experimental_gui: bool,

    /// Extra KML to display
    #[structopt(long = "kml")]
    kml: Option<String>,

    /// Optional savestate to load
    #[structopt(long = "load_from")]
    load_from: Option<String>,

    /// Scenario name for savestating
    #[structopt(long = "scenario_name", default_value = "editor")]
    scenario_name: String,
}

fn main() {
    let flags = Flags::from_args();

    let opengl = OpenGL::V3_2;
    let settings = WindowSettings::new("Editor", [1024, 768])
        .opengl(opengl)
        .exit_on_esc(false)
        // TODO it'd be cool to dynamically tweak antialiasing settings as we zoom in
        .samples(2)
        .srgb(false);
    let window: GlutinWindow = settings.build().expect("Could not create window");
    let events = Events::new(EventSettings::new().lazy(true));
    let gl = GlGraphics::new(opengl);

    let texture_settings = TextureSettings::new().filter(Filter::Nearest);
    let glyphs = GlyphCache::new(
        // TODO don't assume this exists!
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        (),
        texture_settings,
    ).expect("Could not load font");

    let window_size = window.draw_size();
    if flags.experimental_gui {
        run(
            events,
            window,
            gl,
            glyphs,
            experimental::UI::new(window_size),
        );
    } else {
        run(
            events,
            window,
            gl,
            glyphs,
            ui::UI::new(
                &flags.abst_input,
                flags.scenario_name,
                window_size,
                flags.rng_seed,
                flags.kml,
                flags.load_from,
            ),
        );
    }
}

fn run<T: gui::GUI>(
    mut events: Events,
    mut window: GlutinWindow,
    mut gl: GlGraphics,
    mut glyphs: GlyphCache,
    mut gui: T,
) {
    let mut last_event_mode = gui::EventLoopMode::InputOnly;

    while let Some(ev) = events.next(&mut window) {
        let mut input = UserInput::new(ev.clone());
        let new_event_mode = gui.event(&mut input);
        // Don't constantly reset the events struct -- only when laziness changes.
        if new_event_mode != last_event_mode {
            events.set_lazy(new_event_mode == gui::EventLoopMode::InputOnly);
            last_event_mode = new_event_mode;
        }

        if let Some(args) = ev.render_args() {
            gl.draw(args.viewport(), |c, g| {
                gui.draw(
                    &mut ezgui::GfxCtx::new(&mut glyphs, g, c),
                    input,
                    window.draw_size(),
                );
            });
        }
    }
}
