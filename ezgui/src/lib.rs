// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

mod canvas;
mod color;
mod event;
mod input;
mod log_scroller;
mod menu;
mod runner;
mod screen_geom;
mod scrolling_menu;
mod text;
mod text_box;
mod top_menu;
mod wizard;

pub use crate::canvas::{
    Canvas, HorizontalAlignment, VerticalAlignment, BOTTOM_LEFT, CENTERED, TOP_RIGHT,
};
pub use crate::color::Color;
pub use crate::event::{Event, Key};
pub use crate::input::{ModalMenu, UserInput};
pub use crate::log_scroller::LogScroller;
pub use crate::runner::{run, EventLoopMode, GUI};
pub use crate::screen_geom::ScreenPt;
pub use crate::scrolling_menu::ScrollingMenu;
pub use crate::text::Text;
pub use crate::text_box::TextBox;
pub use crate::top_menu::{Folder, TopMenu};
pub use crate::wizard::{Wizard, WrappedWizard};
use geom::Pt2D;
use graphics::Transformed;
use opengl_graphics::GlGraphics;
use std::mem;

pub struct GfxCtx<'a> {
    orig_ctx: graphics::Context,
    ctx: graphics::Context,
    gfx: &'a mut GlGraphics,
}

impl<'a> GfxCtx<'a> {
    pub fn new(g: &'a mut GlGraphics, c: graphics::Context) -> GfxCtx<'a> {
        GfxCtx {
            gfx: g,
            orig_ctx: c,
            ctx: c,
        }
    }

    // Up to the caller to call unfork()!
    // TODO Canvas doesn't understand this change, so things like text drawing that use
    // map_to_screen will just be confusing.
    pub fn fork(&mut self, top_left: Pt2D, zoom: f64) -> graphics::Context {
        mem::replace(
            &mut self.ctx,
            self.orig_ctx
                .trans(-zoom * top_left.x(), -zoom * top_left.y())
                .zoom(zoom),
        )
    }

    pub fn fork_screenspace(&mut self) -> graphics::Context {
        self.fork(Pt2D::new(0.0, 0.0), 1.0)
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
        self.draw_polygon(color, &line.to_polyline().make_polygons_blindly(thickness));
    }

    pub fn draw_rounded_line(&mut self, color: Color, thickness: f64, line: &geom::Line) {
        self.draw_line(color, thickness, line);
        self.draw_circle(color, &geom::Circle::new(line.pt1(), thickness / 2.0));
        self.draw_circle(color, &geom::Circle::new(line.pt2(), thickness / 2.0));
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
                &[
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
    layer_name: String,
    // If None, never automatically enable at a certain zoom level.
    min_zoom: Option<f64>,

    enabled: bool,
}

impl ToggleableLayer {
    pub fn new(layer_name: &str, min_zoom: Option<f64>) -> ToggleableLayer {
        ToggleableLayer {
            min_zoom,
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
        if input.action_chosen(&format!("show/hide {}", self.layer_name)) {
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
