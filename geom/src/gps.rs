use std::fmt;

use anyhow::Result;
use geojson::{GeoJson, Value};
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

    /// Finds the average of a set of coordinates.
    pub fn center(pts: &[LonLat]) -> LonLat {
        if pts.is_empty() {
            panic!("Can't find center of 0 points");
        }
        let mut x = 0.0;
        let mut y = 0.0;
        for pt in pts {
            x += pt.x();
            y += pt.y();
        }
        let len = pts.len() as f64;
        LonLat::new(x / len, y / len)
    }

    /// Parses a WKT-style line-string into a list of coordinates.
    pub fn parse_wkt_linestring(raw: &str) -> Option<Vec<LonLat>> {
        // Input is something like LINESTRING (-111.9263026 33.4245036, -111.9275146 33.4245016,
        // -111.9278751 33.4233106)
        let mut pts = Vec::new();
        // -111.9446 33.425474, -111.9442814 33.4254737, -111.9442762 33.426894
        for pair in raw
            .strip_prefix("LINESTRING (")?
            .strip_suffix(')')?
            .split(", ")
        {
            let mut nums = Vec::new();
            for x in pair.split(' ') {
                nums.push(x.parse::<f64>().ok()?);
            }
            if nums.len() != 2 {
                return None;
            }
            pts.push(LonLat::new(nums[0], nums[1]));
        }
        if pts.len() < 2 {
            return None;
        }
        Some(pts)
    }

    /// Extract polygons from a raw GeoJSON string. For multipolygons, only returns the first
    /// member. If the GeoJSON feature has a property called `name`, this will also be returned.
    pub fn parse_geojson_polygons(raw: String) -> Result<Vec<(Vec<LonLat>, Option<String>)>> {
        let geojson = raw.parse::<GeoJson>()?;
        let features = match geojson {
            GeoJson::Feature(feature) => vec![feature],
            GeoJson::FeatureCollection(feature_collection) => feature_collection.features,
            _ => bail!("Unexpected geojson: {:?}", geojson),
        };
        let mut polygons = Vec::new();
        for mut feature in features {
            let points = match feature.geometry.take().map(|g| g.value) {
                Some(Value::MultiPolygon(multi_polygon)) => multi_polygon[0][0].clone(),
                Some(Value::Polygon(polygon)) => polygon[0].clone(),
                _ => bail!("Unexpected feature: {:?}", feature),
            };
            let name = feature
                .property("name")
                .and_then(|value| value.as_str())
                .map(|x| x.to_string());
            polygons.push((
                points
                    .into_iter()
                    .map(|pt| LonLat::new(pt[0], pt[1]))
                    .collect(),
                name,
            ));
        }
        Ok(polygons)
    }

    /// Reads a GeoJSON file and returns coordinates from the one polygon contained.
    pub fn read_geojson_polygon(path: &str) -> Result<Vec<LonLat>> {
        let raw = fs_err::read_to_string(path)?;
        let mut list = Self::parse_geojson_polygons(raw)?;
        if list.len() != 1 {
            bail!("{path} doesn't contain exactly one polygon");
        }
        Ok(list.pop().unwrap().0)
    }

    pub fn to_geojson(self) -> geojson::Geometry {
        geojson::Geometry::new(geojson::Value::Point(vec![self.x(), self.y()]))
    }
}

impl fmt::Display for LonLat {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "LonLat({0}, {1})", self.x(), self.y())
    }
}

impl From<LonLat> for geo::Point {
    fn from(pt: LonLat) -> Self {
        geo::Point::new(pt.x(), pt.y())
    }
}
