use std::fmt;

use serde::{Deserialize, Serialize};

use abstutil::{deserialize_usize, serialize_usize};
use geom::{Angle, Line, PolyLine, Polygon, Pt2D};

use crate::{osm, Position};

// TODO For now, ignore the mapped roads linking things and just use the same driveway approach
// that buildings use.

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ParkingLotID(
    #[serde(
        serialize_with = "serialize_usize",
        deserialize_with = "deserialize_usize"
    )]
    pub usize,
);

impl fmt::Display for ParkingLotID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Parking lot #{}", self.0)
    }
}

/// Parking lots have some fixed capacity for cars, and are connected to a sidewalk and road.
#[derive(Serialize, Deserialize)]
pub struct ParkingLot {
    pub id: ParkingLotID,
    pub polygon: Polygon,
    pub aisles: Vec<Vec<Pt2D>>,
    pub osm_id: osm::OsmID,
    /// The middle of the "T", pointing towards the parking aisle
    pub spots: Vec<(Pt2D, Angle)>,
    /// If we can't render all spots (maybe a lot with no aisles or a multi-story garage), still
    /// count the other spots.
    pub extra_spots: usize,

    /// Goes from the lot to the driving lane
    pub driveway_line: PolyLine,
    /// Guaranteed to be at least 7m (MAX_CAR_LENGTH + a little buffer) away from both ends of the
    /// lane, to prevent various headaches
    pub driving_pos: Position,

    /// Lot to sidewalk
    pub sidewalk_line: Line,
    pub sidewalk_pos: Position,
}

impl ParkingLot {
    pub fn capacity(&self) -> usize {
        self.spots.len() + self.extra_spots
    }
}
