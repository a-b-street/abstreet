#[derive(Clone, Copy, PartialEq)]
pub struct ScreenPt {
    pub x: f64,
    pub y: f64,
}

impl ScreenPt {
    pub fn new(x: f64, y: f64) -> ScreenPt {
        ScreenPt { x, y }
    }
}

pub struct ScreenRectangle {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
}

impl ScreenRectangle {
    pub fn contains(&self, pt: ScreenPt) -> bool {
        pt.x >= self.x1 && pt.x <= self.x2 && pt.y >= self.y1 && pt.y <= self.y2
    }

    pub fn translate(&self, dx: f64, dy: f64) -> ScreenRectangle {
        ScreenRectangle {
            x1: self.x1 + dx,
            y1: self.y1 + dy,
            x2: self.x2 + dx,
            y2: self.y2 + dy,
        }
    }
}
