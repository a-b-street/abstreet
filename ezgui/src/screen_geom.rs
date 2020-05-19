use crate::Canvas;
use geom::{trim_f64, Polygon, Pt2D};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenPt {
    pub x: f64,
    pub y: f64,
}

impl ScreenPt {
    pub fn new(x: f64, y: f64) -> ScreenPt {
        ScreenPt { x, y }
    }

    // The geom layer operates in map-space, but currently reusing lots of geom abstractions for
    // screen-space.
    pub fn to_pt(self) -> Pt2D {
        Pt2D::new(self.x, self.y)
    }
}

#[derive(Clone, Debug)]
pub struct ScreenRectangle {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
}

impl ScreenRectangle {
    pub fn top_left(top_left: ScreenPt, dims: ScreenDims) -> ScreenRectangle {
        ScreenRectangle {
            x1: top_left.x,
            y1: top_left.y,
            x2: top_left.x + dims.width,
            y2: top_left.y + dims.height,
        }
    }

    pub fn placeholder() -> ScreenRectangle {
        ScreenRectangle {
            x1: 0.0,
            y1: 0.0,
            x2: 0.0,
            y2: 0.0,
        }
    }

    pub fn contains(&self, pt: ScreenPt) -> bool {
        pt.x >= self.x1 && pt.x <= self.x2 && pt.y >= self.y1 && pt.y <= self.y2
    }

    pub fn pt_to_percent(&self, pt: ScreenPt) -> Option<(f64, f64)> {
        if self.contains(pt) {
            Some((
                (pt.x - self.x1) / self.width(),
                (pt.y - self.y1) / self.height(),
            ))
        } else {
            None
        }
    }
    pub fn percent_to_pt(&self, x: f64, y: f64) -> ScreenPt {
        ScreenPt::new(self.x1 + x * self.width(), self.y1 + y * self.height())
    }

    // TODO Remove these in favor of dims()
    pub fn width(&self) -> f64 {
        self.x2 - self.x1
    }

    pub fn height(&self) -> f64 {
        self.y2 - self.y1
    }

    pub fn dims(&self) -> ScreenDims {
        ScreenDims::new(self.x2 - self.x1, self.y2 - self.y1)
    }

    pub fn center(&self) -> ScreenPt {
        ScreenPt::new((self.x1 + self.x2) / 2.0, (self.y1 + self.y2) / 2.0)
    }

    pub fn to_polygon(&self) -> Polygon {
        Polygon::rectangle(self.width(), self.height()).translate(self.x1, self.y1)
    }
}

// TODO Everything screen-space should probably just be usize, can't have fractional pixels?
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ScreenDims {
    pub width: f64,
    pub height: f64,
}

impl ScreenDims {
    pub fn new(width: f64, height: f64) -> ScreenDims {
        ScreenDims {
            width: trim_f64(width),
            height: trim_f64(height),
        }
    }

    pub fn top_left_for_corner(&self, corner: ScreenPt, canvas: &Canvas) -> ScreenPt {
        // TODO Ideally also avoid covered canvas areas
        if corner.x + self.width < canvas.window_width {
            // corner.x is the left corner
            if corner.y + self.height < canvas.window_height {
                // corner.y is the top corner
                corner
            } else {
                // corner.y is the bottom corner
                ScreenPt::new(corner.x, corner.y - self.height)
            }
        } else {
            // corner.x is the right corner
            if corner.y + self.height < canvas.window_height {
                // corner.y is the top corner
                ScreenPt::new(corner.x - self.width, corner.y)
            } else {
                // corner.y is the bottom corner
                ScreenPt::new(corner.x - self.width, corner.y - self.height)
            }
        }
    }
}
