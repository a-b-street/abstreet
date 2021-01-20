use std::fmt;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};

use anyhow::Result;
use ordered_float::NotNan;
use serde::{Deserialize, Serialize};

use crate::{Distance, GPSBounds, Pt2D};

/// Represents a (longitude, latitude) point.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub struct LonLat {
    longitude: NotNan<f64>,
    latitude: NotNan<f64>,
}

impl LonLat {
    /// Note the order of arguments!
    pub fn new(lon: f64, lat: f64) -> LonLat {
        LonLat {
            longitude: NotNan::new(lon).unwrap(),
            latitude: NotNan::new(lat).unwrap(),
        }
    }

    /// Returns the longitude of this point.
    pub fn x(self) -> f64 {
        self.longitude.into_inner()
    }

    /// Returns the latitude of this point.
    pub fn y(self) -> f64 {
        self.latitude.into_inner()
    }

    /// Transform this to a world-space point. Can go out of bounds.
    pub fn to_pt(self, b: &GPSBounds) -> Pt2D {
        let (width, height) = {
            let pt = b.get_max_world_pt();
            (pt.x(), pt.y())
        };

        let x = (self.x() - b.min_lon) / (b.max_lon - b.min_lon) * width;
        // Invert y, so that the northernmost latitude is 0. Screen drawing order, not Cartesian
        // grid.
        let y = height - ((self.y() - b.min_lat) / (b.max_lat - b.min_lat) * height);
        Pt2D::new(x, y)
    }

    /// Returns the Haversine distance to another point.
    pub(crate) fn gps_dist(self, other: LonLat) -> Distance {
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

    /// Pretty meaningless units, for comparing distances very roughly
    pub fn fast_dist(self, other: LonLat) -> NotNan<f64> {
        NotNan::new((self.x() - other.x()).powi(2) + (self.y() - other.y()).powi(2)).unwrap()
    }

    /// Parses a file in the https://wiki.openstreetmap.org/wiki/Osmosis/Polygon_Filter_File_Format
    /// and returns all points.
    pub fn read_osmosis_polygon(path: &str) -> Result<Vec<LonLat>> {
        let f = File::open(path)?;
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
                parts[0].parse::<f64>()?,
                parts[1].parse::<f64>()?,
            ));
        }
        Ok(pts)
    }

    /// Writes a set of points to a file in the
    /// https://wiki.openstreetmap.org/wiki/Osmosis/Polygon_Filter_File_Format. The input should
    /// be a closed ring, with the first and last point matching.
    pub fn write_osmosis_polygon(path: &str, pts: &Vec<LonLat>) -> Result<()> {
        let mut f = File::create(path)?;
        writeln!(f, "boundary")?;
        writeln!(f, "1")?;
        for pt in pts {
            writeln!(f, "     {}    {}", pt.x(), pt.y())?;
        }
        writeln!(f, "END")?;
        writeln!(f, "END")?;
        Ok(())
    }
}

impl fmt::Display for LonLat {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "LonLat({0}, {1})", self.x(), self.y())
    }
}
