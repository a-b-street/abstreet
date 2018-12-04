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
mod color;
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
pub use color::Color;
use geom::Pt2D;
use graphics::character::CharacterCache;
use graphics::Transformed;
pub use input::UserInput;
pub use log_scroller::LogScroller;
pub use menu::Menu;
use opengl_graphics::{GlGraphics, Texture};
use piston::input::Key;
pub use runner::{run, GUI};
use std::mem;
pub use text::{Text, TEXT_FG_COLOR};
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

    // Up to the caller to call unfork()!
    pub fn fork(&mut self, top_left: Pt2D, zoom: f64) -> graphics::Context {
        mem::replace(
            &mut self.ctx,
            self.orig_ctx
                .trans(-zoom * top_left.x(), -zoom * top_left.y())
                .zoom(zoom),
        )
    }

    pub fn unfork(&mut self, old_ctx: graphics::Context) {
        self.ctx = old_ctx;
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
