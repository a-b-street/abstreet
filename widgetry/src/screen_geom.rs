use serde::{Deserialize, Serialize};

use geom::{trim_f64, Polygon, Pt2D};

use crate::{Canvas, EdgeInsets};

/// ScreenPt is in units of logical pixels, as opposed to physical pixels.
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

    pub fn zero() -> Self {
        Self { x: 0.0, y: 0.0 }
    }

    pub fn translated(&self, x: f64, y: f64) -> Self {
        Self {
            x: self.x + x,
            y: self.y + y,
        }
    }
}

impl From<winit::dpi::LogicalPosition<f64>> for ScreenPt {
    fn from(lp: winit::dpi::LogicalPosition<f64>) -> ScreenPt {
        ScreenPt { x: lp.x, y: lp.y }
    }
}

/// ScreenRectangle is in units of logical pixels, as opposed to physical pixels.
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

// REVIEW: Rename to something shorter? e.g. Dims / Size
/// ScreenDims is in units of logical pixels, as opposed to physical pixels.
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

    pub fn zero() -> Self {
        ScreenDims {
            width: 0.0,
            height: 0.0,
        }
    }

    pub fn square(square: f64) -> Self {
        Self::new(square, square)
    }

    pub fn pad(&self, edge_insets: EdgeInsets) -> Self {
        Self {
            width: self.width + edge_insets.left + edge_insets.right,
            height: self.height + edge_insets.top + edge_insets.bottom,
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

impl From<winit::dpi::LogicalSize<f64>> for ScreenDims {
    fn from(size: winit::dpi::LogicalSize<f64>) -> ScreenDims {
        ScreenDims {
            width: size.width,
            height: size.height,
        }
    }
}

impl From<ScreenDims> for winit::dpi::LogicalSize<f64> {
    fn from(dims: ScreenDims) -> winit::dpi::LogicalSize<f64> {
        winit::dpi::LogicalSize::new(dims.width, dims.height)
    }
}

impl From<f64> for ScreenDims {
    fn from(square: f64) -> ScreenDims {
        ScreenDims::square(square)
    }
}

/// (Width, Height) -> ScreenDims
impl From<(f64, f64)> for ScreenDims {
    fn from(width_and_height: (f64, f64)) -> ScreenDims {
        ScreenDims::new(width_and_height.0, width_and_height.1)
    }
}

impl From<geom::Bounds> for ScreenDims {
    fn from(bounds: geom::Bounds) -> Self {
        ScreenDims::new(bounds.width(), bounds.height())
    }
}

impl From<ScreenDims> for stretch::geometry::Size<stretch::style::Dimension> {
    fn from(dims: ScreenDims) -> Self {
        Self {
            width: stretch::style::Dimension::Points(dims.width as f32),
            height: stretch::style::Dimension::Points(dims.height as f32),
        }
    }
}
