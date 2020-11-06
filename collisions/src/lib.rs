//! A simple data format to list collisions that've occurred in the real world. The data is
//! serializable in a binary format or as JSON.

#[macro_use]
extern crate log;

use geom::{Duration, LonLat};
use kml::ExtraShapes;
use serde::{Deserialize, Serialize};

/// A single dataset describing some collisions that happened.
#[derive(Serialize, Deserialize)]
pub struct CollisionDataset {
    /// A URL pointing to the original data source.
    pub source_url: String,
    /// The collisions imported from the data source.
    pub collisions: Vec<Collision>,
}

/// A single collision that occurred in the real world.
#[derive(Serialize, Deserialize)]
pub struct Collision {
    /// A single point describing where the collision occurred.
    pub location: LonLat,
    /// The local time the collision occurred.
    pub time: Duration,
    /// The severity reported in the original data source.
    pub severity: Severity,
    /* TODO Many more interesting and common things: the date, the number of
     * people/vehicles/bikes/casualties, road/weather/alcohol/speeding conditions possibly
     * influencing the event, etc. */
}

/// A simple ranking for how severe the collision was. Different agencies use different
/// classification systems, each of which likely has their own nuance and bias. This is
/// deliberately simplified.
#[derive(Serialize, Deserialize)]
pub enum Severity {
    Slight,
    Serious,
    Fatal,
}

/// Import data from the UK STATS19 dataset. See https://github.com/ropensci/stats19. Any parsing
/// errors will skip the row and log a warning.
pub fn import_stats19(input: ExtraShapes, source_url: &str) -> CollisionDataset {
    let mut data = CollisionDataset {
        source_url: source_url.to_string(),
        collisions: Vec::new(),
    };
    for shape in input.shapes {
        if shape.points.len() != 1 {
            warn!("One row had >1 point: {:?}", shape);
            continue;
        }
        let time = match Duration::parse(&format!("{}:00", shape.attributes["Time"])) {
            Ok(time) => time,
            Err(err) => {
                warn!("Couldn't parse time: {}", err);
                continue;
            }
        };
        let severity = match shape.attributes["Accident_Severity"].as_ref() {
            // TODO Is this backwards?
            "1" => Severity::Slight,
            "2" => Severity::Serious,
            "3" => Severity::Fatal,
            x => {
                warn!("Unknown severity {}", x);
                continue;
            }
        };
        data.collisions.push(Collision {
            location: shape.points[0],
            time,
            severity,
        });
    }
    data
}

/// Import data from Seattle GeoData
/// (https://data-seattlecitygis.opendata.arcgis.com/datasets/5b5c745e0f1f48e7a53acec63a0022ab_0).
/// Any parsing errors will skip the row and log a warning.
pub fn import_seattle(input: ExtraShapes, source_url: &str) -> CollisionDataset {
    let mut data = CollisionDataset {
        source_url: source_url.to_string(),
        collisions: Vec::new(),
    };
    for shape in input.shapes {
        if shape.points.len() != 1 {
            warn!("One row had >1 point: {:?}", shape);
            continue;
        }
        let time = match parse_incdttm(&shape.attributes["INCDTTM"]) {
            Some(time) => time,
            None => {
                warn!("Couldn't parse time {}", shape.attributes["INCDTTM"]);
                continue;
            }
        };
        let severity = match shape
            .attributes
            .get("SEVERITYCODE")
            .cloned()
            .unwrap_or_else(String::new)
            .as_ref()
        {
            "1" | "0" => Severity::Slight,
            "2b" | "2" => Severity::Serious,
            "3" => Severity::Fatal,
            x => {
                warn!("Unknown severity {}", x);
                continue;
            }
        };
        data.collisions.push(Collision {
            location: shape.points[0],
            time,
            severity,
        });
    }
    data
}

// INCDTTM is something like "11/12/2019 7:30:00 AM"
fn parse_incdttm(x: &str) -> Option<Duration> {
    let parts = x.split(" ").collect::<Vec<_>>();
    if parts.len() != 3 {
        return None;
    }
    let time = Duration::parse(parts[1]).ok()?;
    if parts[2] == "AM" {
        Some(time)
    } else if parts[2] == "PM" {
        Some(time + Duration::hours(12))
    } else {
        None
    }
}
