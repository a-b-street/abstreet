// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

extern crate aabb_quadtree;
extern crate graphics;
extern crate opengl_graphics;
extern crate piston;

pub mod canvas;
pub mod input;
mod keys;
pub mod menu;
pub mod text;
pub mod text_box;

use graphics::character::CharacterCache;
use graphics::types::Color;
use opengl_graphics::{GlGraphics, Texture};
use piston::input::Key;

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

    pub fn draw_line(&mut self, style: &graphics::Line, pts: [f64; 4]) {
        style.draw(pts, &self.ctx.draw_state, self.ctx.transform, self.gfx);
    }

    pub fn draw_arrow(&mut self, style: &graphics::Line, pts: [f64; 4], head_size: f64) {
        style.draw_arrow(
            pts,
            head_size,
            &self.ctx.draw_state,
            self.ctx.transform,
            self.gfx,
        );
    }

    pub fn draw_polygon(&mut self, color: Color, pts: &[[f64; 2]]) {
        graphics::Polygon::new(color).draw(pts, &self.ctx.draw_state, self.ctx.transform, self.gfx);
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
    key_name: String,
    // If None, never automatically enable at a certain zoom level.
    min_zoom: Option<f64>,

    enabled: bool,
}

impl ToggleableLayer {
    pub fn new(
        layer_name: &str,
        key: Key,
        key_name: &str,
        min_zoom: Option<f64>,
    ) -> ToggleableLayer {
        ToggleableLayer {
            key,
            min_zoom,
            layer_name: String::from(layer_name),
            key_name: String::from(key_name),
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

    pub fn handle_event(&mut self, input: &mut input::UserInput) -> bool {
        if input.unimportant_key_pressed(
            self.key,
            &format!("Press {} to toggle {}", self.key_name, self.layer_name),
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
