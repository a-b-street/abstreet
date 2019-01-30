use crate::{Bounds, Distance, HashablePt2D, Pt2D};
use aabb_quadtree::geom::{Point, Rect};
use serde_derive::{Deserialize, Serialize};
use std::f64;
use std::fmt;

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

    pub fn gps_dist_meters(&self, other: LonLat) -> Distance {
        // Haversine distance
        let earth_radius_m = 6_371_000.0;
        let lon1 = self.longitude.to_radians();
        let lon2 = other.longitude.to_radians();
        let lat1 = self.latitude.to_radians();
        let lat2 = other.latitude.to_radians();

        let delta_lat = lat2 - lat1;
        let delta_lon = lon2 - lon1;

        let a = (delta_lat / 2.0).sin().powi(2)
            + (delta_lon / 2.0).sin().powi(2) * lat1.cos() * lat2.cos();
        let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
        Distance::meters(earth_radius_m * c)
    }

    pub fn center(pts: &Vec<LonLat>) -> LonLat {
        let mut lon = 0.0;
        let mut lat = 0.0;
        for pt in pts {
            lon += pt.longitude;
            lat += pt.latitude;
        }
        let len = pts.len() as f64;
        LonLat {
            longitude: lon / len,
            latitude: lat / len,
        }
    }

    pub fn to_hashable(&self) -> HashablePt2D {
        HashablePt2D::new(self.longitude, self.latitude)
    }
}

impl fmt::Display for LonLat {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "LonLat({0}, {1})", self.longitude, self.latitude)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GPSBounds {
    pub(crate) min_lon: f64,
    pub(crate) min_lat: f64,
    pub(crate) max_lon: f64,
    pub(crate) max_lat: f64,

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

    // TODO cache this
    pub fn get_max_world_pt(&self) -> Pt2D {
        let width = LonLat::new(self.min_lon, self.min_lat)
            .gps_dist_meters(LonLat::new(self.max_lon, self.min_lat));
        let height = LonLat::new(self.min_lon, self.min_lat)
            .gps_dist_meters(LonLat::new(self.min_lon, self.max_lat));
        Pt2D::new(width.inner(), height.inner())
    }

    pub fn to_bounds(&self) -> Bounds {
        let mut b = Bounds::new();
        b.update(Pt2D::new(0.0, 0.0));
        b.update(self.get_max_world_pt());
        b
    }
}
