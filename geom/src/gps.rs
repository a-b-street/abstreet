use aabb_quadtree::geom::{Point, Rect};
use std::f64;
use HashablePt2D;

// longitude is x, latitude is y
#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct LonLat {
    pub longitude: f64,
    pub latitude: f64,
}

impl LonLat {
    pub fn new(lon: f64, lat: f64) -> LonLat {
        LonLat {
            longitude: lon,
            latitude: lat,
        }
    }

    // TODO use dimensioned?
    pub fn gps_dist_meters(&self, other: LonLat) -> f64 {
        // Haversine distance
        let earth_radius_m = 6371000.0;
        let lon1 = self.longitude.to_radians();
        let lon2 = other.longitude.to_radians();
        let lat1 = self.latitude.to_radians();
        let lat2 = other.latitude.to_radians();

        let delta_lat = lat2 - lat1;
        let delta_lon = lon2 - lon1;

        let a = (delta_lat / 2.0).sin().powi(2)
            + (delta_lon / 2.0).sin().powi(2) * lat1.cos() * lat2.cos();
        let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
        earth_radius_m * c
    }

    pub fn to_hashable(&self) -> HashablePt2D {
        HashablePt2D::new(self.longitude, self.latitude)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GPSBounds {
    pub min_lon: f64,
    pub min_lat: f64,
    pub max_lon: f64,
    pub max_lat: f64,

    // TODO hack to easily construct test maps
    pub represents_world_space: bool,
}

impl GPSBounds {
    pub fn new() -> GPSBounds {
        GPSBounds {
            min_lon: f64::MAX,
            min_lat: f64::MAX,
            max_lon: f64::MIN,
            max_lat: f64::MIN,
            represents_world_space: false,
        }
    }

    pub fn update(&mut self, pt: LonLat) {
        self.min_lon = self.min_lon.min(pt.longitude);
        self.max_lon = self.max_lon.max(pt.longitude);
        self.min_lat = self.min_lat.min(pt.latitude);
        self.max_lat = self.max_lat.max(pt.latitude);
    }

    pub fn contains(&self, pt: LonLat) -> bool {
        pt.longitude >= self.min_lon
            && pt.longitude <= self.max_lon
            && pt.latitude >= self.min_lat
            && pt.latitude <= self.max_lat
    }

    pub fn as_bbox(&self) -> Rect {
        Rect {
            top_left: Point {
                x: self.min_lon as f32,
                y: self.min_lat as f32,
            },
            bottom_right: Point {
                x: self.max_lon as f32,
                y: self.max_lat as f32,
            },
        }
    }
}
