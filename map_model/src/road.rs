use geom::PolyLine;
use std::collections::BTreeMap;
use std::fmt;
use LaneID;

// TODO reconsider pub usize. maybe outside world shouldnt know.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RoadID(pub usize);

impl fmt::Display for RoadID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "RoadID({0})", self.0)
    }
}

// These're bidirectional (possibly)
#[derive(Debug)]
pub struct Road {
    pub id: RoadID,
    pub osm_tags: BTreeMap<String, String>,
    pub osm_way_id: i64,

    // Invariant: A road must contain at least one child
    pub children_forwards: Vec<LaneID>,
    pub children_backwards: Vec<LaneID>,

    // Unshifted center points. Order implies road orientation.
    pub center_pts: PolyLine,
}

impl Road {
    // lane must belong to this road. Offset 0 is the centermost lane on each side of a road, then
    // it counts up from there.
    pub fn lane_offset(&self, lane: LaneID) -> u8 {
        if let Some(idx) = self.children_forwards.iter().position(|l| *l == lane) {
            return idx as u8;
        }
        if let Some(idx) = self.children_backwards.iter().position(|l| *l == lane) {
            return idx as u8;
        }
        panic!("{} doesn't contain {}", self.id, lane);
    }

    // Is this lane the arbitrary canonical lane of this road? Used for deciding who should draw
    // yellow center lines.
    pub fn is_canonical_lane(&self, lane: LaneID) -> bool {
        if !self.children_forwards.is_empty() {
            return lane == self.children_forwards[0];
        }
        lane == self.children_backwards[0]
    }
}
