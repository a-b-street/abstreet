use std::cell::RefCell;
use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use geom::{Bounds, Pt2D};

use crate::{Key, ScreenDims, ScreenPt, ScreenRectangle, UpdateType, UserInput};

// Click and release counts as a normal click, not a drag, if the distance between click and
// release is less than this.
const DRAG_THRESHOLD: f64 = 5.0;

const PAN_SPEED: f64 = 15.0;

const PANNING_THRESHOLD: f64 = 25.0;

pub struct Canvas {
    // All of these f64's are in screen-space, so do NOT use Pt2D.
    // Public for saving/loading... should probably do better
    pub cam_x: f64,
    pub cam_y: f64,
    pub cam_zoom: f64,

    // TODO Should this become Option<ScreenPt>?
    pub(crate) cursor: ScreenPt,
    pub(crate) window_has_cursor: bool,

    // Only for drags starting on the map. Only used to pan the map. (Last event, original)
    pub(crate) drag_canvas_from: Option<(ScreenPt, ScreenPt)>,
    drag_just_ended: bool,

    pub window_width: f64,
    pub window_height: f64,

    // TODO Proper API for setting these
    pub map_dims: (f64, f64),
    pub settings: CanvasSettings,

    // TODO Bit weird and hacky to mutate inside of draw() calls.
    pub(crate) covered_areas: RefCell<Vec<ScreenRectangle>>,

    // Kind of just widgetry state awkwardly stuck here...
    pub(crate) keys_held: HashSet<Key>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct CanvasSettings {
    pub invert_scroll: bool,
    pub touchpad_to_move: bool,
    pub edge_auto_panning: bool,
    pub keys_to_pan: bool,
    pub gui_scroll_speed: usize,
    // TODO Ideally this would be an f64, but elsewhere we use it in a Spinner. Until we override
    // the Display trait to do some rounding, floating point increments render pretty horribly.
    pub canvas_scroll_speed: usize,
    /// Some map-space elements are drawn differently when unzoomed and zoomed. This specifies the canvas
    /// zoom level where they switch. The concept of "unzoomed" and "zoomed" is used by
    /// `ToggleZoomed`.
    pub min_zoom_for_detail: f64,
}

impl CanvasSettings {
    pub fn new() -> CanvasSettings {
        CanvasSettings {
            invert_scroll: false,
            touchpad_to_move: false,
            edge_auto_panning: false,
            keys_to_pan: false,
            gui_scroll_speed: 5,
            canvas_scroll_speed: 10,
            min_zoom_for_detail: 4.0,
        }
    }
}

impl Canvas {
    pub(crate) fn new(initial_dims: ScreenDims, settings: CanvasSettings) -> Canvas {
        Canvas {
            cam_x: 0.0,
            cam_y: 0.0,
            cam_zoom: 1.0,

            cursor: ScreenPt::new(0.0, 0.0),
            window_has_cursor: true,

            drag_canvas_from: None,
            drag_just_ended: false,

            window_width: initial_dims.width,
            window_height: initial_dims.height,

            map_dims: (0.0, 0.0),
            settings,

            covered_areas: RefCell::new(Vec::new()),

            keys_held: HashSet::new(),
        }
    }

    pub fn max_zoom(&self) -> f64 {
        50.0
    }

    pub fn min_zoom(&self) -> f64 {
        let percent_window = 0.8;
        (percent_window * self.window_width / self.map_dims.0)
            .min(percent_window * self.window_height / self.map_dims.1)
    }

    pub fn is_max_zoom(&self) -> bool {
        self.cam_zoom >= self.max_zoom()
    }

    pub fn is_min_zoom(&self) -> bool {
        self.cam_zoom <= self.min_zoom()
    }

    pub(crate) fn handle_event(&mut self, input: &mut UserInput) -> Option<UpdateType> {
        // Can't start dragging or zooming on top of covered area
        if let Some(map_pt) = self.get_cursor_in_map_space() {
            if self.settings.touchpad_to_move {
                if let Some((scroll_x, scroll_y)) = input.get_mouse_scroll() {
                    if self.keys_held.contains(&Key::LeftControl) {
                        self.zoom(scroll_y, self.cursor);
                    } else {
                        // Woo, inversion is different for the two. :P
                        self.cam_x -= scroll_x * PAN_SPEED;
                        self.cam_y -= scroll_y * PAN_SPEED;
                    }
                }
            } else {
                if input.left_mouse_button_pressed() {
                    self.drag_canvas_from = Some((self.get_cursor(), self.get_cursor()));
                }

                if let Some((_, scroll)) = input.get_mouse_scroll() {
                    self.zoom(scroll, self.cursor);
                }
            }

            if self.settings.keys_to_pan {
                if input.pressed(Key::LeftArrow) {
                    self.cam_x -= PAN_SPEED;
                }
                if input.pressed(Key::RightArrow) {
                    self.cam_x += PAN_SPEED;
                }
                if input.pressed(Key::UpArrow) {
                    self.cam_y -= PAN_SPEED;
                }
                if input.pressed(Key::DownArrow) {
                    self.cam_y += PAN_SPEED;
                }
                if input.pressed(Key::Q) {
                    self.zoom(
                        1.0,
                        ScreenPt::new(self.window_width / 2.0, self.window_height / 2.0),
                    );
                }
                if input.pressed(Key::W) {
                    self.zoom(
                        -1.0,
                        ScreenPt::new(self.window_width / 2.0, self.window_height / 2.0),
                    );
                }
            }

            if input.left_mouse_double_clicked() {
                self.zoom(8.0, self.map_to_screen(map_pt));
            }
        }

        // If we start the drag on the map and move the mouse off the map, keep dragging.
        if let Some((click, orig)) = self.drag_canvas_from {
            let pt = self.get_cursor();
            self.cam_x += click.x - pt.x;
            self.cam_y += click.y - pt.y;
            self.drag_canvas_from = Some((pt, orig));

            if input.left_mouse_button_released() {
                let (_, orig) = self.drag_canvas_from.take().unwrap();
                let dist = ((pt.x - orig.x).powi(2) + (pt.y - orig.y).powi(2)).sqrt();
                if dist > DRAG_THRESHOLD {
                    self.drag_just_ended = true;
                }
            }
        } else if self.drag_just_ended {
            self.drag_just_ended = false;
        } else {
            let cursor_screen_pt = self.get_cursor().to_pt();
            let cursor_map_pt = self.screen_to_map(self.get_cursor());
            let inner_bounds = self.get_inner_bounds();
            let map_bounds = self.get_map_bounds();
            if self.settings.edge_auto_panning
                && !inner_bounds.contains(cursor_screen_pt)
                && map_bounds.contains(cursor_map_pt)
                && input.nonblocking_is_update_event().is_some()
            {
                let center_pt = self.center_to_screen_pt().to_pt();
                let displacement_x = cursor_screen_pt.x() - center_pt.x();
                let displacement_y = cursor_screen_pt.y() - center_pt.y();
                let displacement_magnitude =
                    f64::sqrt(displacement_x.powf(2.0) + displacement_y.powf(2.0));
                let displacement_unit_x = displacement_x / displacement_magnitude;
                let displacement_unit_y = displacement_y / displacement_magnitude;
                // Add displacement along each axis
                self.cam_x += displacement_unit_x * PAN_SPEED;
                self.cam_y += displacement_unit_y * PAN_SPEED;
                return Some(UpdateType::Pan);
            }
        }
        None
    }

    pub fn center_zoom(&mut self, delta: f64) {
        self.zoom(delta, self.center_to_screen_pt())
    }

    pub fn zoom(&mut self, delta: f64, focus: ScreenPt) {
        let old_zoom = self.cam_zoom;
        // By popular request, some limits ;)
        self.cam_zoom = 1.1_f64
            .powf(old_zoom.log(1.1) + delta * (self.settings.canvas_scroll_speed as f64 / 10.0))
            .max(self.min_zoom())
            .min(self.max_zoom());

        // Make screen_to_map of the focus point still point to the same thing after
        // zooming.
        self.cam_x = ((self.cam_zoom / old_zoom) * (focus.x + self.cam_x)) - focus.x;
        self.cam_y = ((self.cam_zoom / old_zoom) * (focus.y + self.cam_y)) - focus.y;
    }

    pub(crate) fn start_drawing(&self) {
        self.covered_areas.borrow_mut().clear();
    }

    // TODO Only public for the OSD. :(
    pub fn mark_covered_area(&self, rect: ScreenRectangle) {
        self.covered_areas.borrow_mut().push(rect);
    }

    // Might be hovering anywhere.
    pub fn get_cursor(&self) -> ScreenPt {
        self.cursor
    }

    pub fn get_cursor_in_screen_space(&self) -> Option<ScreenPt> {
        if self.window_has_cursor && self.get_cursor_in_map_space().is_none() {
            Some(self.get_cursor())
        } else {
            None
        }
    }

    pub fn get_cursor_in_map_space(&self) -> Option<Pt2D> {
        if self.window_has_cursor {
            let pt = self.get_cursor();

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

    pub fn map_to_screen(&self, pt: Pt2D) -> ScreenPt {
        ScreenPt::new(
            (pt.x() * self.cam_zoom) - self.cam_x,
            (pt.y() * self.cam_zoom) - self.cam_y,
        )
    }

    // the inner bound tells us whether auto-panning should or should not take place
    fn get_inner_bounds(&self) -> Bounds {
        let mut b = Bounds::new();
        b.update(ScreenPt::new(PANNING_THRESHOLD, PANNING_THRESHOLD).to_pt());
        b.update(
            ScreenPt::new(
                self.window_width - PANNING_THRESHOLD,
                self.window_height - PANNING_THRESHOLD,
            )
            .to_pt(),
        );
        b
    }

    pub fn get_window_dims(&self) -> ScreenDims {
        ScreenDims::new(self.window_width, self.window_height)
    }

    fn get_map_bounds(&self) -> Bounds {
        let mut b = Bounds::new();
        b.update(Pt2D::new(0.0, 0.0));
        b.update(Pt2D::new(self.map_dims.0, self.map_dims.1));
        b
    }

    pub fn get_screen_bounds(&self) -> Bounds {
        let mut b = Bounds::new();
        b.update(self.screen_to_map(ScreenPt::new(0.0, 0.0)));
        b.update(self.screen_to_map(ScreenPt::new(self.window_width, self.window_height)));
        b
    }

    pub(crate) fn align_window(
        &self,
        dims: ScreenDims,
        horiz: HorizontalAlignment,
        vert: VerticalAlignment,
    ) -> ScreenPt {
        let x1 = match horiz {
            HorizontalAlignment::Left => 0.0,
            HorizontalAlignment::LeftInset => INSET,
            HorizontalAlignment::Center => (self.window_width - dims.width) / 2.0,
            HorizontalAlignment::Right => self.window_width - dims.width,
            HorizontalAlignment::RightOf(x) => x,
            HorizontalAlignment::RightInset => self.window_width - dims.width - INSET,
            HorizontalAlignment::Percent(pct) => pct * self.window_width,
            HorizontalAlignment::Centered(x) => x - (dims.width / 2.0),
        };
        let y1 = match vert {
            VerticalAlignment::Top => 0.0,
            VerticalAlignment::TopInset => INSET,
            VerticalAlignment::Center => (self.window_height - dims.height) / 2.0,
            VerticalAlignment::Bottom => self.window_height - dims.height,
            VerticalAlignment::BottomInset => self.window_height - dims.height - INSET,
            // TODO Hack
            VerticalAlignment::BottomAboveOSD => self.window_height - dims.height - 60.0,
            VerticalAlignment::Percent(pct) => pct * self.window_height,
            VerticalAlignment::Above(y) => y - dims.height,
            VerticalAlignment::Below(y) => y,
        };
        ScreenPt::new(x1, y1)
    }

    pub fn is_unzoomed(&self) -> bool {
        self.cam_zoom < self.settings.min_zoom_for_detail
    }

    pub fn is_zoomed(&self) -> bool {
        self.cam_zoom >= self.settings.min_zoom_for_detail
    }

    pub(crate) fn is_dragging(&self) -> bool {
        // This could be called before or after handle_event. So we need to repeat the threshold
        // check here! Alternatively, we could this upfront in runner.
        if self.drag_just_ended {
            return true;
        }
        if let Some((_, orig)) = self.drag_canvas_from {
            let pt = self.get_cursor();
            let dist = ((pt.x - orig.x).powi(2) + (pt.y - orig.y).powi(2)).sqrt();
            if dist > DRAG_THRESHOLD {
                return true;
            }
        }
        false
    }
}

const INSET: f64 = 16.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HorizontalAlignment {
    Left,
    LeftInset,
    Center,
    Right,
    RightOf(f64),
    RightInset,
    Percent(f64),
    Centered(f64),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum VerticalAlignment {
    Top,
    TopInset,
    Center,
    Bottom,
    BottomInset,
    BottomAboveOSD,
    Percent(f64),
    Above(f64),
    Below(f64),
}
