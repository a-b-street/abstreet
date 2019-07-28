use crate::screen_geom::ScreenRectangle;
use crate::{ScreenPt, Text, UserInput};
use geom::{Bounds, Pt2D};
use glium_glyph::glyph_brush::rusttype::Scale;
use glium_glyph::glyph_brush::GlyphCruncher;
use glium_glyph::GlyphBrush;
use std::cell::RefCell;
use std::collections::HashMap;

const ZOOM_SPEED: f64 = 0.1;

pub struct Canvas {
    // All of these f64's are in screen-space, so do NOT use Pt2D.
    // Public for saving/loading... should probably do better
    pub cam_x: f64,
    pub cam_y: f64,
    pub cam_zoom: f64,

    // TODO We probably shouldn't even track screen-space cursor when we don't have the cursor.
    pub(crate) cursor_x: f64,
    pub(crate) cursor_y: f64,
    window_has_cursor: bool,

    left_mouse_drag_from: Option<ScreenPt>,

    pub window_width: f64,
    pub window_height: f64,

    pub(crate) glyphs: RefCell<GlyphBrush<'static, 'static>>,
    line_height_per_font_size: RefCell<HashMap<usize, f64>>,

    // TODO Bit weird and hacky to mutate inside of draw() calls.
    pub(crate) covered_areas: RefCell<Vec<ScreenRectangle>>,

    // Kind of just ezgui state awkwardly stuck here...
    pub(crate) hide_modal_menus: bool,
    pub(crate) lctrl_held: bool,
}

impl Canvas {
    pub(crate) fn new(
        initial_width: f64,
        initial_height: f64,
        glyphs: GlyphBrush<'static, 'static>,
    ) -> Canvas {
        Canvas {
            cam_x: 0.0,
            cam_y: 0.0,
            cam_zoom: 1.0,

            cursor_x: 0.0,
            cursor_y: 0.0,
            window_has_cursor: true,

            left_mouse_drag_from: None,
            window_width: initial_width,
            window_height: initial_height,

            glyphs: RefCell::new(glyphs),
            line_height_per_font_size: RefCell::new(HashMap::new()),
            covered_areas: RefCell::new(Vec::new()),

            hide_modal_menus: false,
            lctrl_held: false,
        }
    }

    pub(crate) fn is_dragging(&self) -> bool {
        self.left_mouse_drag_from.is_some()
    }

    pub fn handle_event(&mut self, input: &mut UserInput) {
        if let Some(pt) = input.get_moved_mouse() {
            self.cursor_x = pt.x;
            self.cursor_y = pt.y;

            if let Some(click) = self.left_mouse_drag_from {
                self.cam_x += click.x - pt.x;
                self.cam_y += click.y - pt.y;
                self.left_mouse_drag_from = Some(pt);
            }
        }
        // Can't start dragging on top of covered area
        if input.left_mouse_button_pressed() && self.get_cursor_in_map_space().is_some() {
            self.left_mouse_drag_from = Some(self.get_cursor_in_screen_space());
        }
        if input.left_mouse_button_released() {
            self.left_mouse_drag_from = None;
        }
        if let Some(scroll) = input.get_mouse_scroll() {
            // Zoom slower at low zooms, faster at high.
            let delta = scroll * ZOOM_SPEED * self.cam_zoom;
            self.zoom_towards_mouse(delta);
        }
        if input.window_gained_cursor() {
            self.window_has_cursor = true;
        }
        if input.window_lost_cursor() {
            self.window_has_cursor = false;
        }
    }

    pub(crate) fn start_drawing(&self) {
        self.covered_areas.borrow_mut().clear();
    }

    pub fn mark_covered_area(&self, rect: ScreenRectangle) {
        self.covered_areas.borrow_mut().push(rect);
    }

    fn zoom_towards_mouse(&mut self, delta_zoom: f64) {
        let old_zoom = self.cam_zoom;
        self.cam_zoom += delta_zoom;
        if self.cam_zoom <= ZOOM_SPEED {
            self.cam_zoom = ZOOM_SPEED;
        }

        // Make screen_to_map of cursor_{x,y} still point to the same thing after zooming.
        self.cam_x = ((self.cam_zoom / old_zoom) * (self.cursor_x + self.cam_x)) - self.cursor_x;
        self.cam_y = ((self.cam_zoom / old_zoom) * (self.cursor_y + self.cam_y)) - self.cursor_y;
    }

    pub fn get_cursor_in_screen_space(&self) -> ScreenPt {
        ScreenPt::new(self.cursor_x, self.cursor_y)
    }

    pub fn get_cursor_in_map_space(&self) -> Option<Pt2D> {
        if self.window_has_cursor {
            let pt = self.get_cursor_in_screen_space();

            for rect in self.covered_areas.borrow().iter() {
                if rect.contains(pt) {
                    return None;
                }
            }

            Some(self.screen_to_map(pt))
        } else {
            None
        }
    }

    pub fn screen_to_map(&self, pt: ScreenPt) -> Pt2D {
        Pt2D::new(
            (pt.x + self.cam_x) / self.cam_zoom,
            (pt.y + self.cam_y) / self.cam_zoom,
        )
    }

    pub fn center_to_screen_pt(&self) -> ScreenPt {
        ScreenPt::new(self.window_width / 2.0, self.window_height / 2.0)
    }

    pub fn center_to_map_pt(&self) -> Pt2D {
        self.screen_to_map(self.center_to_screen_pt())
    }

    pub fn center_on_map_pt(&mut self, pt: Pt2D) {
        self.cam_x = (pt.x() * self.cam_zoom) - (self.window_width / 2.0);
        self.cam_y = (pt.y() * self.cam_zoom) - (self.window_height / 2.0);
    }

    pub(crate) fn map_to_screen(&self, pt: Pt2D) -> ScreenPt {
        ScreenPt::new(
            (pt.x() * self.cam_zoom) - self.cam_x,
            (pt.y() * self.cam_zoom) - self.cam_y,
        )
    }

    pub fn get_screen_bounds(&self) -> Bounds {
        let mut b = Bounds::new();
        b.update(self.screen_to_map(ScreenPt::new(0.0, 0.0)));
        b.update(self.screen_to_map(ScreenPt::new(self.window_width, self.window_height)));
        b
    }

    pub fn text_dims(&self, txt: &Text) -> (f64, f64) {
        txt.dims(self)
    }

    // Don't call this while glyphs are mutably borrowed.
    pub(crate) fn line_height(&self, font_size: usize) -> f64 {
        let mut hash = self.line_height_per_font_size.borrow_mut();
        if hash.contains_key(&font_size) {
            return hash[&font_size];
        }
        let vmetrics = self.glyphs.borrow().fonts()[0].v_metrics(Scale::uniform(font_size as f32));
        // TODO This works for this font, but could be more paranoid with abs()
        let line_height = f64::from(vmetrics.ascent - vmetrics.descent + vmetrics.line_gap);
        hash.insert(font_size, line_height);
        line_height
    }
}

pub enum HorizontalAlignment {
    Left,
    Center,
    Right,
    FillScreen,
}

pub enum VerticalAlignment {
    Top,
    Center,
    Bottom,
}

pub const BOTTOM_LEFT: (HorizontalAlignment, VerticalAlignment) =
    (HorizontalAlignment::Left, VerticalAlignment::Bottom);
pub const CENTERED: (HorizontalAlignment, VerticalAlignment) =
    (HorizontalAlignment::Center, VerticalAlignment::Center);
