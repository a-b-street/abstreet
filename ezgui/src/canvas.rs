use crate::{Color, ScreenPt, ScreenRectangle, Text, UserInput};
use abstutil::Timer;
use geom::{Bounds, Pt2D};
use glium::texture::Texture2d;
use glium_glyph::glyph_brush::rusttype::Scale;
use glium_glyph::glyph_brush::GlyphCruncher;
use glium_glyph::GlyphBrush;
use serde_derive::{Deserialize, Serialize};
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
    pub(crate) window_has_cursor: bool,

    pub(crate) left_mouse_drag_from: Option<ScreenPt>,

    pub window_width: f64,
    pub window_height: f64,

    pub(crate) screenspace_glyphs: RefCell<GlyphBrush<'static, 'static>>,
    pub(crate) mapspace_glyphs: RefCell<GlyphBrush<'static, 'static>>,
    line_height_per_font_size: RefCell<HashMap<usize, f64>>,

    // TODO Bit weird and hacky to mutate inside of draw() calls.
    pub(crate) covered_areas: RefCell<Vec<ScreenRectangle>>,

    // Kind of just ezgui state awkwardly stuck here...
    pub(crate) lctrl_held: bool,

    // TODO Definitely a weird place to stash this!
    pub(crate) textures: Vec<(String, Texture2d)>,
    pub(crate) texture_lookups: HashMap<String, Color>,
    // Of the default font size
    pub line_height: f64,
    pub(crate) font_size: usize,
}

impl Canvas {
    pub(crate) fn new(
        initial_width: f64,
        initial_height: f64,
        screenspace_glyphs: GlyphBrush<'static, 'static>,
        mapspace_glyphs: GlyphBrush<'static, 'static>,
        font_size: usize,
    ) -> Canvas {
        let mut c = Canvas {
            cam_x: 0.0,
            cam_y: 0.0,
            cam_zoom: 1.0,

            cursor_x: 0.0,
            cursor_y: 0.0,
            window_has_cursor: true,

            left_mouse_drag_from: None,
            window_width: initial_width,
            window_height: initial_height,

            screenspace_glyphs: RefCell::new(screenspace_glyphs),
            mapspace_glyphs: RefCell::new(mapspace_glyphs),
            line_height_per_font_size: RefCell::new(HashMap::new()),
            covered_areas: RefCell::new(Vec::new()),

            lctrl_held: false,

            textures: Vec::new(),
            texture_lookups: HashMap::new(),
            line_height: 0.0,
            font_size,
        };
        c.line_height = c.line_height(c.font_size);
        c
    }

    pub(crate) fn is_dragging(&self) -> bool {
        self.left_mouse_drag_from.is_some()
    }

    pub fn handle_event(&mut self, input: &mut UserInput) {
        // Can't start dragging or zooming on top of covered area
        let mouse_on_map = self.get_cursor_in_map_space().is_some();
        if input.left_mouse_button_pressed() && mouse_on_map {
            self.left_mouse_drag_from = Some(self.get_cursor_in_screen_space());
        }
        if input.left_mouse_button_released() {
            self.left_mouse_drag_from = None;
        }
        if mouse_on_map {
            if let Some(scroll) = input.get_mouse_scroll() {
                // Zoom slower at low zooms, faster at high.
                let delta = scroll * ZOOM_SPEED * self.cam_zoom;
                self.zoom_towards_mouse(delta);
            }
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

    // TODO Maybe return ScreenDims
    pub fn text_dims(&self, txt: &Text) -> (f64, f64) {
        txt.dims(self)
    }

    // Don't call this while screenspace_glyphs is mutably borrowed.
    pub(crate) fn line_height(&self, font_size: usize) -> f64 {
        let mut hash = self.line_height_per_font_size.borrow_mut();
        if hash.contains_key(&font_size) {
            return hash[&font_size];
        }
        let vmetrics =
            self.screenspace_glyphs.borrow().fonts()[0].v_metrics(Scale::uniform(font_size as f32));
        // TODO This works for this font, but could be more paranoid with abs()
        let line_height = f64::from(vmetrics.ascent - vmetrics.descent + vmetrics.line_gap);
        hash.insert(font_size, line_height);
        line_height
    }

    pub fn texture(&self, filename: &str) -> Color {
        if let Some(c) = self.texture_lookups.get(filename) {
            return *c;
        }
        panic!("Don't know texture {}", filename);
    }

    pub fn save_camera_state(&self, map_name: &str) {
        let state = CameraState {
            cam_x: self.cam_x,
            cam_y: self.cam_y,
            cam_zoom: self.cam_zoom,
        };
        let path = abstutil::path_camera_state(map_name);
        abstutil::write_json(&path, &state).unwrap();
        println!("Saved {}", path);
    }

    // True if this succeeds
    pub fn load_camera_state(&mut self, map_name: &str) -> bool {
        let path = abstutil::path_camera_state(map_name);
        match abstutil::read_json::<CameraState>(&path, &mut Timer::throwaway()) {
            Ok(ref loaded) => {
                println!("Loaded {}", path);
                self.cam_x = loaded.cam_x;
                self.cam_y = loaded.cam_y;
                self.cam_zoom = loaded.cam_zoom;
                true
            }
            _ => false,
        }
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

#[derive(Serialize, Deserialize, Debug)]
pub struct CameraState {
    cam_x: f64,
    cam_y: f64,
    cam_zoom: f64,
}
