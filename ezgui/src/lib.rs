// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

extern crate abstutil;
extern crate geom;
extern crate glutin_window;
extern crate graphics;
#[macro_use]
extern crate log;
extern crate opengl_graphics;
extern crate palette;
extern crate piston;
#[macro_use]
extern crate serde_derive;

mod canvas;
mod input;
mod keys;
mod log_scroller;
mod menu;
mod runner;
mod text;
mod text_box;
mod tree_menu;
mod wizard;

pub use canvas::{
    Canvas, HorizontalAlignment, VerticalAlignment, BOTTOM_LEFT, CENTERED, TOP_RIGHT,
};
use graphics::character::CharacterCache;
pub use input::UserInput;
pub use log_scroller::LogScroller;
pub use menu::Menu;
use opengl_graphics::{GlGraphics, Texture};
use piston::input::Key;
pub use runner::{run, GUI};
use std::fmt;
pub use text::Text;
pub use text_box::TextBox;
pub use wizard::{Wizard, WrappedWizard};

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
        graphics::clear(color.0, self.gfx);
    }

    // Use graphics::Line internally for now, but make it easy to switch to something else by
    // picking this API now.
    pub fn draw_line(&mut self, color: Color, thickness: f64, line: &geom::Line) {
        graphics::Line::new(color.0, thickness).draw(
            line_to_array(line),
            &self.ctx.draw_state,
            self.ctx.transform,
            self.gfx,
        );
    }

    pub fn draw_rounded_line(&mut self, color: Color, thickness: f64, line: &geom::Line) {
        graphics::Line::new_round(color.0, thickness).draw(
            line_to_array(line),
            &self.ctx.draw_state,
            self.ctx.transform,
            self.gfx,
        );
    }

    pub fn draw_arrow(&mut self, color: Color, thickness: f64, head_size: f64, line: &geom::Line) {
        graphics::Line::new(color.0, thickness).draw_arrow(
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
        graphics::Line::new_round(color.0, thickness).draw_arrow(
            line_to_array(line),
            head_size,
            &self.ctx.draw_state,
            self.ctx.transform,
            self.gfx,
        );
    }

    pub fn draw_polygon(&mut self, color: Color, poly: &geom::Polygon) {
        for tri in &poly.triangles {
            graphics::Polygon::new(color.0).draw(
                &vec![
                    [tri.pt1.x(), tri.pt1.y()],
                    [tri.pt2.x(), tri.pt2.y()],
                    [tri.pt3.x(), tri.pt3.y()],
                ],
                &self.ctx.draw_state,
                self.ctx.transform,
                self.gfx,
            );
        }
    }

    pub fn draw_circle(&mut self, color: Color, circle: &geom::Circle) {
        graphics::Ellipse::new(color.0).draw(
            [
                circle.center.x() - circle.radius,
                circle.center.y() - circle.radius,
                2.0 * circle.radius,
                2.0 * circle.radius,
            ],
            &self.ctx.draw_state,
            self.ctx.transform,
            self.gfx,
        );
    }
}

pub struct ToggleableLayer {
    category: String,
    layer_name: String,
    key: Key,
    // If None, never automatically enable at a certain zoom level.
    min_zoom: Option<f64>,

    enabled: bool,
}

impl ToggleableLayer {
    pub fn new(
        category: &str,
        layer_name: &str,
        key: Key,
        min_zoom: Option<f64>,
    ) -> ToggleableLayer {
        ToggleableLayer {
            key,
            min_zoom,
            category: category.to_string(),
            layer_name: layer_name.to_string(),
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
            &self.category,
            &format!("toggle {}", self.layer_name),
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

fn line_to_array(l: &geom::Line) -> [f64; 4] {
    [l.pt1().x(), l.pt1().y(), l.pt2().x(), l.pt2().y()]
}

pub enum InputResult<T: Clone> {
    Canceled,
    StillActive,
    Done(String, T),
}

// Copy could be reconsidered, but eh
// TODO only pub so we can construct constants elsewhere. need const fn.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Color(pub [f32; 4]);

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Color(r={}, g={}, b={}, a={})",
            self.0[0], self.0[1], self.0[2], self.0[3]
        )
    }
}

impl Color {
    pub const BLACK: Color = Color([0.0, 0.0, 0.0, 1.0]);
    pub const WHITE: Color = Color([1.0, 1.0, 1.0, 1.0]);
    pub const RED: Color = Color([1.0, 0.0, 0.0, 1.0]);
    pub const GREEN: Color = Color([0.0, 1.0, 0.0, 1.0]);
    pub const BLUE: Color = Color([0.0, 0.0, 1.0, 1.0]);
    pub const CYAN: Color = Color([0.0, 1.0, 1.0, 1.0]);
    pub const YELLOW: Color = Color([1.0, 1.0, 0.0, 1.0]);
    pub const PURPLE: Color = Color([0.5, 0.0, 0.5, 1.0]);

    // TODO should assert stuff about the inputs

    pub fn rgb(r: usize, g: usize, b: usize) -> Color {
        Color::rgba(r, g, b, 1.0)
    }

    pub fn rgb_f(r: f32, g: f32, b: f32) -> Color {
        Color([r, g, b, 1.0])
    }

    pub fn rgba(r: usize, g: usize, b: usize, a: f32) -> Color {
        Color([
            (r as f32) / 255.0,
            (g as f32) / 255.0,
            (b as f32) / 255.0,
            a,
        ])
    }

    pub fn rgba_f(r: f32, g: f32, b: f32, a: f32) -> Color {
        Color([r, g, b, a])
    }

    pub fn grey(f: f32) -> Color {
        Color([f, f, f, 1.0])
    }

    // Deterministically shift a color's brightness based on an ID.
    pub fn shift(&self, id: usize) -> Color {
        use palette::Shade;

        // TODO this needs tuning. too easy to get too light/dark, but also too easy to have too few
        // variants. should maybe just manually come up with a list of 100 colors, hardcode in, modulo.
        let variants = 10;
        let half_variants = variants / 2;
        let modulo = id % variants;
        let scale = 1.0 / (variants as f32);

        let color = palette::Srgb::new(self.0[0], self.0[1], self.0[2]).into_linear();
        let new_color = if modulo < half_variants {
            color.lighten(scale * (modulo as f32))
        } else {
            color.darken(scale * ((modulo - half_variants) as f32))
        };
        Color([new_color.red, new_color.green, new_color.blue, 1.0])
    }

    pub fn alpha(&self, a: f32) -> Color {
        Color([self.0[0], self.0[1], self.0[2], a])
    }
}
