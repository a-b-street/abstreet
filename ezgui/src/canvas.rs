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

use aabb_quadtree::geom::{Point, Rect};
use graphics;
use graphics::{Context, Image, Transformed};
use graphics::character::CharacterCache;
use graphics::types::Color;
use piston::input::{Button, Event, Key, MouseButton, MouseCursorEvent, MouseScrollEvent,
                    PressEvent, ReleaseEvent};
use piston::window::Size;
use opengl_graphics::{GlGraphics, Texture};

//pub const WHITE: Color = [1.0, 1.0, 1.0, 1.0];
pub const BLACK: Color = [0.0, 0.0, 0.0, 1.0];
pub const BLUE: Color = [0.0, 0.0, 1.0, 1.0];
pub const YELLOW: Color = [1.0, 1.0, 0.0, 1.0];
pub const ORANGE: Color = [1.0, 0.65, 0.0, 1.0];
pub const RED: Color = [1.0, 0.0, 0.0, 1.0];
pub const GREEN: Color = [0.0, 1.0, 0.0, 1.0];
pub const LIGHT_GREY: Color = [0.7, 0.7, 0.7, 1.0];
pub const DARK_GREY: Color = [0.3, 0.3, 0.3, 1.0];
pub const PURPLE: Color = [0.5, 0.0, 0.5, 1.0];
pub const CYAN: Color = [0.0, 1.0, 1.0, 1.0];
// TODO it'd be a bit more efficient to not render it at all...
pub const ALMOST_INVISIBLE: Color = [0.0, 0.0, 0.0, 0.1];
//pub const INVISIBLE: Color = [0.0, 0.0, 0.0, 0.0];

const TEXT_FG_COLOR: Color = BLACK;
const TEXT_BG_COLOR: Color = [0.0, 1.0, 0.0, 0.5];

const ZOOM_SPEED: f64 = 0.05;
const PAN_SPEED: f64 = 10.0;

const FONT_SIZE: u32 = 24;
// TODO this is a hack, need a glyphs.height() method as well!
const LINE_HEIGHT: f64 = 22.0;

//struct GfxCtx<'a, G: 'a + Graphics, C: 'a + CharacterCache<Texture = G::Texture>> {
pub struct GfxCtx<'a> {
    pub glyphs: &'a mut CharacterCache<Texture = Texture, Error = String>,
    pub orig_ctx: Context,
    pub ctx: Context,
    pub gfx: &'a mut GlGraphics,
    pub window_size: Size,
}

pub struct Canvas {
    pub cam_x: f64,
    pub cam_y: f64,
    pub cam_zoom: f64,

    cursor_x: f64,
    cursor_y: f64,

    left_mouse_drag_from: Option<[f64; 2]>,
}

impl Canvas {
    pub fn new() -> Canvas {
        Canvas {
            cam_x: 0.0,
            cam_y: 0.0,
            cam_zoom: 1.0,

            cursor_x: 0.0,
            cursor_y: 0.0,

            left_mouse_drag_from: None,
        }
    }

    pub fn is_dragging(&self) -> bool {
        self.left_mouse_drag_from.is_some()
    }

    pub fn handle_event(&mut self, ev: &Event) {
        if let Some(pos) = ev.mouse_cursor_args() {
            self.cursor_x = pos[0];
            self.cursor_y = pos[1];

            if let Some(click) = self.left_mouse_drag_from {
                self.cam_x += click[0] - pos[0];
                self.cam_y += click[1] - pos[1];
                self.left_mouse_drag_from = Some(pos);
            }
        }
        if let Some(Button::Mouse(MouseButton::Left)) = ev.press_args() {
            self.left_mouse_drag_from = Some([self.cursor_x, self.cursor_y]);
        }
        if let Some(Button::Mouse(MouseButton::Left)) = ev.release_args() {
            self.left_mouse_drag_from = None;
        }
        if let Some(Button::Keyboard(key)) = ev.press_args() {
            match key {
                Key::Up => self.cam_y -= PAN_SPEED,
                Key::Down => self.cam_y += PAN_SPEED,
                Key::Left => self.cam_x -= PAN_SPEED,
                Key::Right => self.cam_x += PAN_SPEED,
                Key::Q => self.zoom_towards_mouse(-ZOOM_SPEED),
                Key::W => self.zoom_towards_mouse(ZOOM_SPEED),
                _ => {}
            }
        }
        if let Some(scroll) = ev.mouse_scroll_args() {
            self.zoom_towards_mouse(scroll[1] * ZOOM_SPEED);
        }
    }

    pub fn get_transformed_context(&self, ctx: &Context) -> Context {
        ctx.trans(-self.cam_x, -self.cam_y).zoom(self.cam_zoom)
    }

    pub fn draw_mouse_tooltip(&self, g: &mut GfxCtx, lines: &[String]) {
        let (width, height) = self.text_dims(g, lines);
        let x1 = self.cursor_x - (width / 2.0);
        let y1 = self.cursor_y - (height / 2.0);
        self.draw_text_bubble(g, lines, x1, y1);
    }

    // at the bottom-left of the screen
    pub fn draw_osd_notification(&self, g: &mut GfxCtx, lines: &[String]) {
        if lines.is_empty() {
            return;
        }
        let (_, height) = self.text_dims(g, lines);
        let y1 = f64::from(g.window_size.height) - height;
        self.draw_text_bubble(g, lines, 0.0, y1);
    }

    pub fn draw_text_at(&self, g: &mut GfxCtx, lines: &[String], x: f64, y: f64) {
        self.draw_text_bubble(g, lines, self.map_to_screen_x(x), self.map_to_screen_y(y));
    }

    fn draw_text_bubble(&self, g: &mut GfxCtx, lines: &[String], x1: f64, y1: f64) {
        let (width, height) = self.text_dims(g, lines);
        let tooltip = graphics::Rectangle::new(TEXT_BG_COLOR);
        tooltip.draw(
            [x1, y1, width, height],
            &g.orig_ctx.draw_state,
            g.orig_ctx.transform,
            g.gfx,
        );

        let text = Image::new_color(TEXT_FG_COLOR);
        let mut y = y1 + LINE_HEIGHT;
        for line in lines.iter() {
            let mut x = x1;
            for ch in line.chars() {
                if let Ok(draw_ch) = g.glyphs.character(FONT_SIZE, ch) {
                    text.draw(
                        draw_ch.texture,
                        &g.orig_ctx.draw_state,
                        g.orig_ctx
                            .transform
                            .trans(x + draw_ch.left(), y - draw_ch.top()),
                        g.gfx,
                    );
                    x += draw_ch.width();
                }
            }
            y += LINE_HEIGHT;
        }
    }

    fn text_dims(&self, g: &mut GfxCtx, lines: &[String]) -> (f64, f64) {
        let longest_line = lines.iter().max_by_key(|l| l.len()).unwrap();
        let width = g.glyphs.width(FONT_SIZE, longest_line).unwrap();
        let height = (lines.len() as f64) * LINE_HEIGHT;
        (width, height)
    }

    fn zoom_towards_mouse(&mut self, delta_zoom: f64) {
        let old_zoom = self.cam_zoom;
        self.cam_zoom += delta_zoom;
        if self.cam_zoom <= ZOOM_SPEED {
            self.cam_zoom = ZOOM_SPEED;
        }

        // Make screen_to_map_{x,y} of cursor_{x,y} still point to the same thing after zooming.
        self.cam_x = ((self.cam_zoom / old_zoom) * (self.cursor_x + self.cam_x)) - self.cursor_x;
        self.cam_y = ((self.cam_zoom / old_zoom) * (self.cursor_y + self.cam_y)) - self.cursor_y;
    }

    pub fn get_cursor_in_map_space(&self) -> (f64, f64) {
        (
            self.screen_to_map_x(self.cursor_x),
            self.screen_to_map_y(self.cursor_y),
        )
    }

    fn screen_to_map_x(&self, x: f64) -> f64 {
        (x + self.cam_x) / self.cam_zoom
    }
    fn screen_to_map_y(&self, y: f64) -> f64 {
        (y + self.cam_y) / self.cam_zoom
    }

    pub fn center_on_map_pt(&mut self, x: f64, y: f64, window_size: &Size) {
        self.cam_x = (x * self.cam_zoom) - (f64::from(window_size.width) / 2.0);
        self.cam_y = (y * self.cam_zoom) - (f64::from(window_size.height) / 2.0);
    }

    fn map_to_screen_x(&self, x: f64) -> f64 {
        (x * self.cam_zoom) - self.cam_x
    }
    fn map_to_screen_y(&self, y: f64) -> f64 {
        (y * self.cam_zoom) - self.cam_y
    }

    // little weird to return an aabb_quadtree type here. need standard geometry types
    pub fn get_screen_bbox(&self, window_size: &Size) -> Rect {
        Rect {
            top_left: Point {
                x: self.screen_to_map_x(0.0) as f32,
                y: self.screen_to_map_y(0.0) as f32,
            },
            bottom_right: Point {
                x: self.screen_to_map_x(f64::from(window_size.width)) as f32,
                y: self.screen_to_map_y(f64::from(window_size.height)) as f32,
            },
        }
    }
}

// TODO split a separate module
pub fn color_to_svg(c: Color) -> String {
    format!(
        "rgba({}, {}, {}, {})",
        255.0 * c[0],
        255.0 * c[1],
        255.0 * c[2],
        255.0 * c[3]
    )
}
