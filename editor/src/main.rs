// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

extern crate aabb_quadtree;
extern crate abstutil;
extern crate control;
extern crate dimensioned;
extern crate ezgui;
extern crate glutin_window;
extern crate graphics;
extern crate map_model;
extern crate multimap;
extern crate opengl_graphics;
extern crate ordered_float;
extern crate piston;
extern crate rand;
#[macro_use]
extern crate serde_derive;
extern crate sim;
#[macro_use]
extern crate structopt;
extern crate strum;
#[macro_use]
extern crate strum_macros;

use ezgui::input::UserInput;
use glutin_window::GlutinWindow;
use opengl_graphics::{Filter, GlGraphics, GlyphCache, OpenGL, TextureSettings};
use piston::event_loop::{EventLoop, EventSettings, Events};
use piston::input::RenderEvent;
use piston::window::{Window, WindowSettings};
use structopt::StructOpt;

mod animation;
mod colors;
mod experimental;
mod gui;
mod plugins;
mod render;
mod ui;

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

    let size = &window.draw_size();
    if flags.experimental_gui {
        run(events, window, gl, glyphs, experimental::UI::new());
    } else {
        run(
            events,
            window,
            gl,
            glyphs,
            ui::UI::new(&flags.abst_input, size, flags.rng_seed),
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
    let mut last_event_mode = animation::EventLoopMode::InputOnly;

    while let Some(ev) = events.next(&mut window) {
        let mut input = UserInput::new(ev.clone());
        let (new_gui, new_event_mode) = gui.event(&mut input, &window.draw_size());
        gui = new_gui;
        // Don't constantly reset the events struct -- only when laziness changes.
        if new_event_mode != last_event_mode {
            events.set_lazy(new_event_mode == animation::EventLoopMode::InputOnly);
            last_event_mode = new_event_mode;
        }

        if let Some(args) = ev.render_args() {
            gl.draw(args.viewport(), |c, g| {
                gui.draw(
                    &mut ezgui::GfxCtx {
                        glyphs: &mut glyphs,
                        gfx: g,
                        orig_ctx: c,
                        ctx: c,
                        window_size: window.draw_size(),
                    },
                    input,
                );
            });
        }
    }
}
