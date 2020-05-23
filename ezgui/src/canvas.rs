use crate::assets::Assets;
use crate::{ScreenDims, ScreenPt, ScreenRectangle, UserInput};
use abstutil::Timer;
use geom::{Bounds, Pt2D};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;

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

    // TODO We probably shouldn't even track screen-space cursor when we don't have the cursor.
    pub(crate) cursor_x: f64,
    pub(crate) cursor_y: f64,
    pub(crate) window_has_cursor: bool,

    // Only for drags starting on the map. Only used to pan the map. (Last event, original)
    pub(crate) drag_canvas_from: Option<(ScreenPt, ScreenPt)>,
    pub(crate) drag_just_ended: bool,

    pub window_width: f64,
    pub window_height: f64,

    // TODO Proper API for setting these
    pub map_dims: (f64, f64),
    pub invert_scroll: bool,
    pub touchpad_to_move: bool,
    pub edge_auto_panning: bool,

    // TODO Bit weird and hacky to mutate inside of draw() calls.
    pub(crate) covered_areas: RefCell<Vec<ScreenRectangle>>,

    // Kind of just ezgui state awkwardly stuck here...
    pub(crate) lctrl_held: bool,
    pub(crate) lshift_held: bool,
}

impl Canvas {
    pub(crate) fn new(initial_width: f64, initial_height: f64) -> Canvas {
        Canvas {
            cam_x: 0.0,
            cam_y: 0.0,
            cam_zoom: 1.0,

            cursor_x: 0.0,
            cursor_y: 0.0,
            window_has_cursor: true,

            drag_canvas_from: None,
            drag_just_ended: false,

            window_width: initial_width,
            window_height: initial_height,

            map_dims: (0.0, 0.0),
            invert_scroll: false,
            touchpad_to_move: false,
            edge_auto_panning: false,

            covered_areas: RefCell::new(Vec::new()),

            lctrl_held: false,
            lshift_held: false,
        }
    }

    pub fn min_zoom(&self) -> f64 {
        let percent_window = 0.8;
        (percent_window * self.window_width / self.map_dims.0)
            .min(percent_window * self.window_height / self.map_dims.1)
    }

    pub(crate) fn handle_event(&mut self, input: &mut UserInput) {
        // Can't start dragging or zooming on top of covered area
        if self.get_cursor_in_map_space().is_some() {
            if self.touchpad_to_move {
                if let Some((scroll_x, scroll_y)) = input.get_mouse_scroll() {
                    if self.lctrl_held {
                        let old_zoom = self.cam_zoom;
                        // By popular request, some limits ;)
                        self.cam_zoom = 1.1_f64
                            .powf(old_zoom.log(1.1) + scroll_y)
                            .max(self.min_zoom())
                            .min(150.0);

                        // Make screen_to_map of cursor_{x,y} still point to the same thing after
                        // zooming.
                        self.cam_x = ((self.cam_zoom / old_zoom) * (self.cursor_x + self.cam_x))
                            - self.cursor_x;
                        self.cam_y = ((self.cam_zoom / old_zoom) * (self.cursor_y + self.cam_y))
                            - self.cursor_y;
                    } else {
                        // Woo, inversion is different for the two. :P
                        self.cam_x += scroll_x * PAN_SPEED;
                        self.cam_y -= scroll_y * PAN_SPEED;
                    }
                }
            } else {
                if input.left_mouse_button_pressed() {
                    self.drag_canvas_from = Some((self.get_cursor(), self.get_cursor()));
                }

                if let Some((_, scroll)) = input.get_mouse_scroll() {
                    let old_zoom = self.cam_zoom;
                    // By popular request, some limits ;)
                    self.cam_zoom = 1.1_f64
                        .powf(old_zoom.log(1.1) + scroll)
                        .max(self.min_zoom())
                        .min(150.0);

                    // Make screen_to_map of cursor_{x,y} still point to the same thing after
                    // zooming.
                    self.cam_x =
                        ((self.cam_zoom / old_zoom) * (self.cursor_x + self.cam_x)) - self.cursor_x;
                    self.cam_y =
                        ((self.cam_zoom / old_zoom) * (self.cursor_y + self.cam_y)) - self.cursor_y;
                }
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
            if !inner_bounds.contains(cursor_screen_pt)
                && self.edge_auto_panning
                && map_bounds.contains(cursor_map_pt)
            {
                let center_pt = self.center_to_screen_pt().to_pt();
                let displacement_x = cursor_screen_pt.x() - center_pt.x();
                let displacement_y = cursor_screen_pt.y() - center_pt.y();
                let displacement_magnitude =
                    f64::sqrt(displacement_x.powf(2.0) + displacement_y.powf(2.0));
                let displacement_unit_x = displacement_x / displacement_magnitude;
                let displacement_unit_y = displacement_y / displacement_magnitude;
                //Add displacement along each axis
                self.cam_x += displacement_unit_x * PAN_SPEED;
                self.cam_y += displacement_unit_y * PAN_SPEED;
            }
        }
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
        ScreenPt::new(self.cursor_x, self.cursor_y)
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

    //the inner bound tells us whether auto-panning should or should not take place
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

    pub fn save_camera_state(&self, map_name: &str) {
        let state = CameraState {
            cam_x: self.cam_x,
            cam_y: self.cam_y,
            cam_zoom: self.cam_zoom,
        };
        abstutil::write_json(abstutil::path_camera_state(map_name), &state);
    }

    // True if this succeeds
    pub fn load_camera_state(&mut self, map_name: &str) -> bool {
        match abstutil::maybe_read_json::<CameraState>(
            abstutil::path_camera_state(map_name),
            &mut Timer::throwaway(),
        ) {
            Ok(ref loaded) => {
                self.cam_x = loaded.cam_x;
                self.cam_y = loaded.cam_y;
                self.cam_zoom = loaded.cam_zoom;
                true
            }
            _ => false,
        }
    }

    pub(crate) fn align_window(
        &self,
        assets: &Assets,
        dims: ScreenDims,
        horiz: HorizontalAlignment,
        vert: VerticalAlignment,
    ) -> ScreenPt {
        let x1 = match horiz {
            HorizontalAlignment::Left => 0.0,
            HorizontalAlignment::Center => (self.window_width - dims.width) / 2.0,
            HorizontalAlignment::Right => self.window_width - dims.width,
            HorizontalAlignment::Percent(pct) => pct * self.window_width,
            HorizontalAlignment::Centered(x) => x - (dims.width / 2.0),
        };
        let y1 = match vert {
            VerticalAlignment::Top => 0.0,
            VerticalAlignment::Center => (self.window_height - dims.height) / 2.0,
            VerticalAlignment::Bottom => self.window_height - dims.height,
            // TODO Hack
            VerticalAlignment::BottomAboveOSD => {
                self.window_height - dims.height - 60.0 * *assets.scale_factor.borrow()
            }
            VerticalAlignment::Percent(pct) => pct * self.window_height,
            VerticalAlignment::Above(y) => y - dims.height,
            VerticalAlignment::Below(y) => y,
        };
        ScreenPt::new(x1, y1)
    }
}

#[derive(Clone, Copy)]
pub enum HorizontalAlignment {
    Left,
    Center,
    Right,
    Percent(f64),
    Centered(f64),
}

#[derive(Clone, Copy)]
pub enum VerticalAlignment {
    Top,
    Center,
    Bottom,
    BottomAboveOSD,
    Percent(f64),
    Above(f64),
    Below(f64),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CameraState {
    cam_x: f64,
    cam_y: f64,
    cam_zoom: f64,
}
