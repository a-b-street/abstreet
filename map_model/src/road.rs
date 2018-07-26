use geom::PolyLine;
use std::collections::BTreeMap;
use std::fmt;
use {LaneID, LaneType};

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
    pub children_forwards: Vec<(LaneID, LaneType)>,
    pub children_backwards: Vec<(LaneID, LaneType)>,
    // TODO should consider having a redundant lookup from LaneID

    // Unshifted center points. Order implies road orientation.
    pub center_pts: PolyLine,
}

impl PartialEq for Road {
    fn eq(&self, other: &Road) -> bool {
        self.id == other.id
    }
}

impl Road {
    pub fn edit_lane_type(&mut self, lane: LaneID, new_type: LaneType) {
        let (dir, offset) = self.dir_and_offset(lane);
        if dir {
            self.children_forwards[offset] = (lane, new_type);
        } else {
            self.children_backwards[offset] = (lane, new_type);
        }
    }

    pub fn get_lane_types(&self) -> (Vec<LaneType>, Vec<LaneType>) {
        (
            self.children_forwards.iter().map(|pair| pair.1).collect(),
            self.children_backwards.iter().map(|pair| pair.1).collect(),
        )
    }

    // lane must belong to this road. Offset 0 is the centermost lane on each side of a road, then
    // it counts up from there. Returns true for the forwards direction, false for backwards.
    pub fn dir_and_offset(&self, lane: LaneID) -> (bool, usize) {
        if let Some(idx) = self.children_forwards
            .iter()
            .position(|pair| pair.0 == lane)
        {
            return (true, idx);
        }
        if let Some(idx) = self.children_backwards
            .iter()
            .position(|pair| pair.0 == lane)
        {
            return (false, idx);
        }
        panic!("{} doesn't contain {}", self.id, lane);
    }

    // Is this lane the arbitrary canonical lane of this road? Used for deciding who should draw
    // yellow center lines.
    pub fn is_canonical_lane(&self, lane: LaneID) -> bool {
        if !self.children_forwards.is_empty() {
            return lane == self.children_forwards[0].0;
        }
        lane == self.children_backwards[0].0
    }

    pub fn find_driving_lane(&self, parking: LaneID) -> Option<LaneID> {
        // TODO find the closest one to the parking lane, if there are multiple
        //assert_eq!(l.lane_type, LaneType::Parking);
        self.get_siblings(parking)
            .iter()
            .find(|pair| pair.1 == LaneType::Driving)
            .map(|pair| pair.0)
    }

    pub fn find_parking_lane(&self, driving: LaneID) -> Option<LaneID> {
        //assert_eq!(l.lane_type, LaneType::Driving);
        self.get_siblings(driving)
            .iter()
            .find(|pair| pair.1 == LaneType::Parking)
            .map(|pair| pair.0)
    }

    pub fn get_opposite_lane(&self, lane: LaneID, lane_type: LaneType) -> Option<LaneID> {
        let forwards: Vec<LaneID> = self.children_forwards
            .iter()
            .filter(|pair| pair.1 == lane_type)
            .map(|pair| pair.0)
            .collect();
        let backwards: Vec<LaneID> = self.children_backwards
            .iter()
            .filter(|pair| pair.1 == lane_type)
            .map(|pair| pair.0)
            .collect();

        if let Some(idx) = forwards.iter().position(|id| *id == lane) {
            return backwards.get(idx).map(|id| *id);
        }
        if let Some(idx) = backwards.iter().position(|id| *id == lane) {
            return forwards.get(idx).map(|id| *id);
        }
        panic!("{} doesn't contain {}", self.id, lane);
    }

    fn get_siblings(&self, lane: LaneID) -> &Vec<(LaneID, LaneType)> {
        // TODO rm lane from this list?
        if self.children_forwards
            .iter()
            .find(|pair| pair.0 == lane)
            .is_some()
        {
            return &self.children_forwards;
        }
        if self.children_backwards
            .iter()
            .find(|pair| pair.0 == lane)
            .is_some()
        {
            return &self.children_backwards;
        }
        panic!("{} doesn't contain {}", self.id, lane);
    }
}
