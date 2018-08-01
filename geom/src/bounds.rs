use std::f64;
use {LonLat, Pt2D};

// TODO argh, use this in kml too
// TODO this maybe represents LonLat only?
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Bounds {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

impl Bounds {
    pub fn new() -> Bounds {
        Bounds {
            min_x: f64::MAX,
            min_y: f64::MAX,
            max_x: f64::MIN,
            max_y: f64::MIN,
        }
    }

    pub fn update(&mut self, x: f64, y: f64) {
        self.min_x = self.min_x.min(x);
        self.max_x = self.max_x.max(x);
        self.min_y = self.min_y.min(y);
        self.max_y = self.max_y.max(y);
    }

    pub fn update_pt(&mut self, pt: &Pt2D) {
        self.update(pt.x(), pt.y());
    }

    pub fn update_coord(&mut self, coord: &LonLat) {
        self.update(coord.longitude, coord.latitude);
    }

    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.min_x && x <= self.max_x && y >= self.min_y && y <= self.max_y
    }
}
