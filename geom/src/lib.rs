#![allow(clippy::new_without_default)]

#[macro_use]
extern crate anyhow;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub use crate::angle::Angle;
pub use crate::bounds::{Bounds, GPSBounds, QuadTree, QuadTreeBuilder};
pub use crate::circle::Circle;
pub use crate::distance::Distance;
pub use crate::duration::Duration;
pub use crate::find_closest::FindClosest;
pub use crate::gps::LonLat;
pub use crate::line::{InfiniteLine, Line};
pub use crate::percent::Percent;
pub use crate::polygon::Polygon;
pub use crate::polyline::{ArrowCap, PolyLine};
pub use crate::pt::{HashablePt2D, Pt2D};
pub use crate::ring::Ring;
pub use crate::speed::Speed;
pub use crate::stats::{HgramValue, Histogram, Statistic};
pub use crate::tessellation::{Tessellation, Triangle};
pub use crate::time::Time;

mod angle;
mod bounds;
mod circle;
mod conversions;
mod distance;
mod duration;
mod find_closest;
mod gps;
mod line;
mod percent;
mod polygon;
mod polyline;
mod pt;
mod ring;
mod speed;
mod stats;
mod tessellation;
mod time;
mod utils;

// About 0.4 inches... which is quite tiny on the scale of things. :)
pub const EPSILON_DIST: Distance = Distance::const_meters(0.01);

/// Reduce the precision of an f64. This helps ensure serialization is idempotent (everything is
/// exactly the same before and after saving/loading). Ideally we'd use some kind of proper
/// fixed-precision type instead of f64.
pub fn trim_f64(x: f64) -> f64 {
    (x * 10_000.0).round() / 10_000.0
}

/// Serializes a trimmed `f64` as an `i32` to save space.
fn serialize_f64<S: Serializer>(x: &f64, s: S) -> Result<S::Ok, S::Error> {
    // So a trimmed f64's range becomes 2**31 / 10,000 =~ 214,000, which is plenty
    // We MUST round here, the same as trim_f64. The unit test demonstrates why.
    let int = (x * 10_000.0).round() as i32;
    int.serialize(s)
}

/// Deserializes a trimmed `f64` from an `i32`.
fn deserialize_f64<'de, D: Deserializer<'de>>(d: D) -> Result<f64, D::Error> {
    let x = <i32>::deserialize(d)?;
    Ok(x as f64 / 10_000.0)
}

/// Specifies how to stringify different geom objects.
#[derive(Clone, Serialize, Deserialize, Copy)]
pub struct UnitFmt {
    /// Round `Duration`s to a whole number of seconds.
    pub round_durations: bool,
    /// Display in metric; US imperial otherwise.
    pub metric: bool,
}

impl UnitFmt {
    /// Default settings using metric.
    pub fn metric() -> Self {
        Self {
            round_durations: true,
            metric: true,
        }
    }

    /// Default settings using imperial.
    pub fn imperial() -> Self {
        Self {
            round_durations: true,
            metric: false,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CornerRadii {
    pub top_left: f64,
    pub top_right: f64,
    pub bottom_right: f64,
    pub bottom_left: f64,
}

impl CornerRadii {
    pub fn uniform(radius: f64) -> Self {
        Self {
            top_left: radius,
            top_right: radius,
            bottom_right: radius,
            bottom_left: radius,
        }
    }

    pub fn zero() -> Self {
        Self::uniform(0.0)
    }
}

impl std::convert::From<f64> for CornerRadii {
    fn from(uniform: f64) -> Self {
        Self::uniform(uniform)
    }
}

impl Default for CornerRadii {
    fn default() -> Self {
        Self::zero()
    }
}

/// Create a GeoJson with one feature per geometry, with the specified properties.
// TODO Rethink after https://github.com/georust/geojson/issues/170
pub fn geometries_with_properties_to_geojson(
    input: Vec<(
        geojson::Geometry,
        serde_json::Map<String, serde_json::Value>,
    )>,
) -> geojson::GeoJson {
    let mut features = Vec::new();
    for (geom, properties) in input {
        let mut f = geojson::Feature::from(geom);
        f.properties = Some(properties);
        features.push(f);
    }
    geojson::GeoJson::from(geojson::FeatureCollection {
        bbox: None,
        features,
        foreign_members: None,
    })
}

/// Create a GeoJson with one feature per geometry, and no properties.
pub fn geometries_to_geojson(input: Vec<geojson::Geometry>) -> geojson::GeoJson {
    let mut features = Vec::new();
    for geom in input {
        features.push(geojson::Feature::from(geom));
    }
    geojson::GeoJson::from(geojson::FeatureCollection {
        bbox: None,
        features,
        foreign_members: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{Rng, SeedableRng};

    #[test]
    fn f64_trimming() {
        let mut rng = rand_xorshift::XorShiftRng::seed_from_u64(42);
        for _ in 0..1_000 {
            // Generate a random point
            let input = Pt2D::new(
                rng.gen_range(-214_000.00..214_000.0),
                rng.gen_range(-214_000.00..214_000.0),
            );
            // Round-trip to JSON and bincode
            let json_roundtrip: Pt2D =
                serde_json::from_slice(serde_json::to_string(&input).unwrap().as_bytes()).unwrap();
            let bincode_roundtrip: Pt2D =
                bincode::deserialize(&bincode::serialize(&input).unwrap()).unwrap();

            // Make sure everything is EXACTLY equal
            if !exactly_eq(input, json_roundtrip) || !exactly_eq(input, bincode_roundtrip) {
                panic!("Round-tripping mismatch!\ninput=            {:?}\njson_roundtrip=   {:?}\nbincode_roundtrip={:?}", input,json_roundtrip, bincode_roundtrip);
            }
        }

        // Hardcode a particular case, where we can hand-verify that it trims to 4 decimal places
        let input = Pt2D::new(1.2345678, 9.876543);
        let json_roundtrip: Pt2D =
            serde_json::from_slice(serde_json::to_string(&input).unwrap().as_bytes()).unwrap();
        let bincode_roundtrip: Pt2D =
            bincode::deserialize(&bincode::serialize(&input).unwrap()).unwrap();
        assert_eq!(input.x(), 1.2346);
        assert_eq!(input.y(), 9.8765);
        assert!(exactly_eq(input, json_roundtrip));
        assert!(exactly_eq(input, bincode_roundtrip));
    }

    // Don't use the PartialEq implementation, which does an epsilon check
    fn exactly_eq(pt1: Pt2D, pt2: Pt2D) -> bool {
        pt1.x() == pt2.x() && pt1.y() == pt2.y()
    }
}
