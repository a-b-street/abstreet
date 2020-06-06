use crate::Position;
use geom::{Angle, Line, PolyLine, Polygon, Pt2D};
use serde::{Deserialize, Serialize};
use std::fmt;

// TODO For now, ignore the mapped roads linking things and just use the same driveway approach
// that buildings use.

// TODO Nits:
// - handle relations with individual slots, like https://www.openstreetmap.org/relation/2580595?
// - Northlake: onstreet or a lot?
// - E1 at UW filtered out
// - aisle clipping isnt perfect (23rd and rainier, pepsi)

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ParkingLotID(pub usize);

impl fmt::Display for ParkingLotID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Parking lot #{}", self.0)
    }
}

#[derive(Serialize, Deserialize)]
pub struct ParkingLot {
    pub id: ParkingLotID,
    pub polygon: Polygon,
    pub aisles: Vec<Vec<Pt2D>>,
    // TODO Rename this
    pub capacity: Option<usize>,
    pub osm_id: i64,
    // The middle of the "T", pointing towards the parking aisle
    pub spots: Vec<(Pt2D, Angle)>,

    // Goes from the lot to the driving lane
    pub driveway_line: PolyLine,
    // Guaranteed to be at least 7m (MAX_CAR_LENGTH + a little buffer) away from both ends of the
    // lane, to prevent various headaches
    pub driving_pos: Position,

    // Lot to sidewalk
    pub sidewalk_line: Line,
    pub sidewalk_pos: Position,
}
