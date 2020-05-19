use crate::Distance;
use ordered_float::NotNan;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs::File;
use std::io::{BufRead, BufReader, Error, ErrorKind};

// longitude is x, latitude is y
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub struct LonLat {
    longitude: NotNan<f64>,
    latitude: NotNan<f64>,
}

impl LonLat {
    pub fn new(lon: f64, lat: f64) -> LonLat {
        LonLat {
            longitude: NotNan::new(lon).unwrap(),
            latitude: NotNan::new(lat).unwrap(),
        }
    }

    pub fn x(self) -> f64 {
        self.longitude.into_inner()
    }

    pub fn y(self) -> f64 {
        self.latitude.into_inner()
    }

    pub fn gps_dist_meters(self, other: LonLat) -> Distance {
        // Haversine distance
        let earth_radius_m = 6_371_000.0;
        let lon1 = self.x().to_radians();
        let lon2 = other.x().to_radians();
        let lat1 = self.y().to_radians();
        let lat2 = other.y().to_radians();

        let delta_lat = lat2 - lat1;
        let delta_lon = lon2 - lon1;

        let a = (delta_lat / 2.0).sin().powi(2)
            + (delta_lon / 2.0).sin().powi(2) * lat1.cos() * lat2.cos();
        let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
        Distance::meters(earth_radius_m * c)
    }

    // Pretty meaningless units, for comparing distances very roughly
    pub fn fast_dist(self, other: LonLat) -> NotNan<f64> {
        NotNan::new((self.x() - other.x()).powi(2) + (self.y() - other.y()).powi(2)).unwrap()
    }

    pub(crate) fn approx_eq(self, other: LonLat) -> bool {
        let epsilon = 1e-8;
        (self.x() - other.x()).abs() < epsilon && (self.y() - other.y()).abs() < epsilon
    }

    pub fn read_osmosis_polygon(path: String) -> Result<Vec<LonLat>, Error> {
        let f = File::open(&path)?;
        let mut pts = Vec::new();
        for (idx, line) in BufReader::new(f).lines().enumerate() {
            if idx < 2 {
                continue;
            }
            let line = line?;
            if line == "END" {
                break;
            }
            let parts = line.trim().split("    ").collect::<Vec<_>>();
            pts.push(LonLat::new(
                parts[0]
                    .parse::<f64>()
                    .map_err(|err| Error::new(ErrorKind::Other, err))?,
                parts[1]
                    .parse::<f64>()
                    .map_err(|err| Error::new(ErrorKind::Other, err))?,
            ));
        }
        Ok(pts)
    }
}

impl fmt::Display for LonLat {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "LonLat({0}, {1})", self.x(), self.y())
    }
}
