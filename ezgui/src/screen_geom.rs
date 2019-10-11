use crate::Canvas;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenPt {
    pub x: f64,
    pub y: f64,
}

impl ScreenPt {
    pub fn new(x: f64, y: f64) -> ScreenPt {
        ScreenPt { x, y }
    }
}

#[derive(Clone)]
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

    pub fn contains(&self, pt: ScreenPt) -> bool {
        pt.x >= self.x1 && pt.x <= self.x2 && pt.y >= self.y1 && pt.y <= self.y2
    }

    pub fn width(&self) -> f64 {
        self.x2 - self.x1
    }

    pub fn height(&self) -> f64 {
        self.y2 - self.y1
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScreenDims {
    pub width: f64,
    pub height: f64,
}

impl ScreenDims {
    pub fn new(width: f64, height: f64) -> ScreenDims {
        ScreenDims { width, height }
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
