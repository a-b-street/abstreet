// Copyright 2018 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

extern crate aabb_quadtree;
extern crate control;
extern crate ezgui;
extern crate geom;
extern crate glutin_window;
extern crate graphics;
extern crate map_model;
extern crate multimap;
extern crate opengl_graphics;
extern crate ordered_float;
extern crate piston;
#[macro_use]
extern crate serde_derive;
extern crate sim;
#[macro_use]
extern crate structopt;
extern crate vecmath;

use ezgui::input::UserInput;
use glutin_window::GlutinWindow;
use opengl_graphics::{Filter, GlGraphics, GlyphCache, OpenGL, TextureSettings};
use piston::event_loop::{EventLoop, EventSettings, Events};
use piston::input::RenderEvent;
use piston::window::{Window, WindowSettings};
use structopt::StructOpt;

mod animation;
mod plugins;
mod render;
mod savestate;
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
    let mut window: GlutinWindow = settings.build().expect("Could not create window");
    let mut events = Events::new(EventSettings::new().lazy(true));
    let mut gl = GlGraphics::new(opengl);

    let texture_settings = TextureSettings::new().filter(Filter::Nearest);
    let glyphs = &mut GlyphCache::new(
        // TODO don't assume this exists!
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        (),
        texture_settings,
    ).expect("Could not load font");

    let mut ui = ui::UI::new(&flags.abst_input, &window.draw_size(), flags.rng_seed);
    let mut last_event_mode = animation::EventLoopMode::InputOnly;

    while let Some(ev) = events.next(&mut window) {
        let mut input = UserInput::new(ev.clone());
        let (new_ui, new_event_mode) = ui.event(&mut input, &window.draw_size());
        ui = new_ui;
        // Don't constantly reset the events struct -- only when laziness changes.
        if new_event_mode != last_event_mode {
            events.set_lazy(new_event_mode == animation::EventLoopMode::InputOnly);
            last_event_mode = new_event_mode;
        }

        if let Some(args) = ev.render_args() {
            gl.draw(args.viewport(), |c, g| {
                use graphics::clear;

                clear([1.0; 4], g);

                ui.draw(
                    &mut ezgui::canvas::GfxCtx {
                        glyphs,
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
