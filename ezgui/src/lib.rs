// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

extern crate aabb_quadtree;
extern crate geom;
extern crate glutin_window;
extern crate graphics;
extern crate opengl_graphics;
extern crate palette;
extern crate piston;

mod canvas;
mod input;
mod keys;
mod menu;
mod runner;
mod text;
mod text_box;

pub use canvas::Canvas;
use graphics::character::CharacterCache;
use graphics::types::Color;
pub use input::UserInput;
pub use menu::{Menu, MenuResult};
use opengl_graphics::{GlGraphics, Texture};
use piston::input::Key;
pub use runner::{run, EventLoopMode, GUI};
pub use text_box::TextBox;

//struct GfxCtx<'a, G: 'a + Graphics, C: 'a + CharacterCache<Texture = G::Texture>> {
pub struct GfxCtx<'a> {
    glyphs: &'a mut CharacterCache<Texture = Texture, Error = String>,
    orig_ctx: graphics::Context,
    ctx: graphics::Context,
    gfx: &'a mut GlGraphics,
}

impl<'a> GfxCtx<'a> {
    pub fn new(
        glyphs: &'a mut CharacterCache<Texture = Texture, Error = String>,
        g: &'a mut GlGraphics,
        c: graphics::Context,
    ) -> GfxCtx<'a> {
        GfxCtx {
            glyphs: glyphs,
            gfx: g,
            orig_ctx: c,
            ctx: c,
        }
    }

    pub fn clear(&mut self, color: Color) {
        graphics::clear(color, self.gfx);
    }

    // Use graphics::Line internally for now, but make it easy to switch to something else by
    // picking this API now.
    pub fn draw_line(&mut self, color: Color, thickness: f64, line: &geom::Line) {
        graphics::Line::new(color, thickness).draw(
            line_to_array(line),
            &self.ctx.draw_state,
            self.ctx.transform,
            self.gfx,
        );
    }

    pub fn draw_rounded_line(&mut self, color: Color, thickness: f64, line: &geom::Line) {
        graphics::Line::new_round(color, thickness).draw(
            line_to_array(line),
            &self.ctx.draw_state,
            self.ctx.transform,
            self.gfx,
        );
    }

    pub fn draw_arrow(&mut self, color: Color, thickness: f64, head_size: f64, line: &geom::Line) {
        graphics::Line::new(color, thickness).draw_arrow(
            line_to_array(line),
            head_size,
            &self.ctx.draw_state,
            self.ctx.transform,
            self.gfx,
        );
    }

    pub fn draw_rounded_arrow(
        &mut self,
        color: Color,
        thickness: f64,
        head_size: f64,
        line: &geom::Line,
    ) {
        graphics::Line::new_round(color, thickness).draw_arrow(
            line_to_array(line),
            head_size,
            &self.ctx.draw_state,
            self.ctx.transform,
            self.gfx,
        );
    }

    pub fn draw_polygon(&mut self, color: Color, poly: &geom::Polygon) {
        for pts in poly.for_drawing().iter() {
            graphics::Polygon::new(color).draw(
                pts,
                &self.ctx.draw_state,
                self.ctx.transform,
                self.gfx,
            );
        }
    }

    pub fn draw_ellipse(&mut self, color: Color, ellipse: [f64; 4]) {
        graphics::Ellipse::new(color).draw(
            ellipse,
            &self.ctx.draw_state,
            self.ctx.transform,
            self.gfx,
        );
    }

    pub fn draw_rectangle(&mut self, color: Color, rect: [f64; 4]) {
        graphics::Rectangle::new(color).draw(
            rect,
            &self.ctx.draw_state,
            self.ctx.transform,
            self.gfx,
        );
    }
}

pub struct ToggleableLayer {
    layer_name: String,
    key: Key,
    // If None, never automatically enable at a certain zoom level.
    min_zoom: Option<f64>,

    enabled: bool,
}

impl ToggleableLayer {
    pub fn new(layer_name: &str, key: Key, min_zoom: Option<f64>) -> ToggleableLayer {
        ToggleableLayer {
            key,
            min_zoom,
            layer_name: String::from(layer_name),
            enabled: false,
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn handle_zoom(&mut self, before_zoom: f64, after_zoom: f64) {
        if let Some(threshold) = self.min_zoom {
            let before_value = before_zoom >= threshold;
            let after_value = after_zoom >= threshold;
            if before_value != after_value {
                self.enabled = after_value;
            }
        }
    }

    // True if there was a change
    pub fn event(&mut self, input: &mut input::UserInput) -> bool {
        if input.unimportant_key_pressed(
            self.key,
            &format!(
                "Press {} to toggle {}",
                keys::describe_key(self.key),
                self.layer_name
            ),
        ) {
            self.enabled = !self.enabled;
            return true;
        }
        false
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }
}

// Deterministically shift a color's brightness based on an ID.
pub fn shift_color(c: Color, id: usize) -> Color {
    use palette::Shade;

    // TODO this needs tuning. too easy to get too light/dark, but also too easy to have too few
    // variants. should maybe just manually come up with a list of 100 colors, hardcode in, modulo.
    let variants = 10;
    let half_variants = variants / 2;
    let modulo = id % variants;
    let scale = 1.0 / (variants as f32);

    let color = palette::Srgb::new(c[0], c[1], c[2]).into_linear();
    let new_color = if modulo < half_variants {
        color.lighten(scale * (modulo as f32))
    } else {
        color.darken(scale * ((modulo - half_variants) as f32))
    };
    [new_color.red, new_color.green, new_color.blue, 1.0]
}

fn line_to_array(l: &geom::Line) -> [f64; 4] {
    [l.pt1().x(), l.pt1().y(), l.pt2().x(), l.pt2().y()]
}
