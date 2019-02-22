use crate::{Distance, HashablePt2D};
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
