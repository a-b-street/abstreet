// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use aabb_quadtree::geom::{Point, Rect};
use graphics::Transformed;
use piston::input::{
    Button, Event, MouseButton, MouseCursorEvent, MouseScrollEvent, PressEvent, ReleaseEvent,
};
use piston::window::Size;
use text;
use GfxCtx;

const ZOOM_SPEED: f64 = 0.1;

pub struct Canvas {
    pub cam_x: f64,
    pub cam_y: f64,
    pub cam_zoom: f64,

    cursor_x: f64,
    cursor_y: f64,

    left_mouse_drag_from: Option<[f64; 2]>,

    pub window_size: Size,
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
            window_size: Size {
                width: 0,
                height: 0,
            },
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
        if let Some(scroll) = ev.mouse_scroll_args() {
            // Zoom slower at low zooms, faster at high.
            let delta = scroll[1] * ZOOM_SPEED * self.cam_zoom;
            self.zoom_towards_mouse(delta);
        }
    }

    pub fn start_drawing(&mut self, g: &mut GfxCtx, window_size: Size) {
        self.window_size = window_size;
        g.ctx = g.orig_ctx
            .trans(-self.cam_x, -self.cam_y)
            .zoom(self.cam_zoom)
    }

    pub fn draw_mouse_tooltip(&self, g: &mut GfxCtx, lines: &[String]) {
        let (width, height) = text::dims(g, lines);
        let x1 = self.cursor_x - (width / 2.0);
        let y1 = self.cursor_y - (height / 2.0);
        text::draw_text_bubble(g, lines, x1, y1);
    }

    // at the bottom-left of the screen
    pub fn draw_osd_notification(&self, g: &mut GfxCtx, lines: &[String]) {
        if lines.is_empty() {
            return;
        }
        let (_, height) = text::dims(g, lines);
        let y1 = f64::from(self.window_size.height) - height;
        text::draw_text_bubble(g, lines, 0.0, y1);
    }

    pub fn draw_text_at(&self, g: &mut GfxCtx, lines: &[String], x: f64, y: f64) {
        text::draw_text_bubble(g, lines, self.map_to_screen_x(x), self.map_to_screen_y(y));
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

    pub fn screen_to_map_x(&self, x: f64) -> f64 {
        (x + self.cam_x) / self.cam_zoom
    }
    pub fn screen_to_map_y(&self, y: f64) -> f64 {
        (y + self.cam_y) / self.cam_zoom
    }

    pub fn center_on_map_pt(&mut self, x: f64, y: f64) {
        self.cam_x = (x * self.cam_zoom) - (f64::from(self.window_size.width) / 2.0);
        self.cam_y = (y * self.cam_zoom) - (f64::from(self.window_size.height) / 2.0);
    }

    fn map_to_screen_x(&self, x: f64) -> f64 {
        (x * self.cam_zoom) - self.cam_x
    }
    fn map_to_screen_y(&self, y: f64) -> f64 {
        (y * self.cam_zoom) - self.cam_y
    }

    // little weird to return an aabb_quadtree type here. need standard geometry types
    pub fn get_screen_bbox(&self) -> Rect {
        Rect {
            top_left: Point {
                x: self.screen_to_map_x(0.0) as f32,
                y: self.screen_to_map_y(0.0) as f32,
            },
            bottom_right: Point {
                x: self.screen_to_map_x(f64::from(self.window_size.width)) as f32,
                y: self.screen_to_map_y(f64::from(self.window_size.height)) as f32,
            },
        }
    }
}
