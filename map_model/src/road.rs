use geom::PolyLine;
use std::collections::BTreeMap;
use std::fmt;
use {LaneID, LaneType, Map};

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
    // TODO should consider having a redundant lookup from LaneID to direction and lane type

    // Unshifted center points. Order implies road orientation.
    pub center_pts: PolyLine,
}

impl PartialEq for Road {
    fn eq(&self, other: &Road) -> bool {
        self.id == other.id
    }
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

    pub fn find_driving_lane(&self, parking: LaneID, map: &Map) -> Option<LaneID> {
        // TODO find the closest one to the parking lane, if there are multiple
        //assert_eq!(l.lane_type, LaneType::Parking);
        self.get_siblings(parking)
            .iter()
            .find(|&&id| map.get_l(id).lane_type == LaneType::Driving)
            .map(|id| *id)
    }

    pub fn find_parking_lane(&self, driving: LaneID, map: &Map) -> Option<LaneID> {
        //assert_eq!(l.lane_type, LaneType::Driving);
        self.get_siblings(driving)
            .iter()
            .find(|&&id| map.get_l(id).lane_type == LaneType::Parking)
            .map(|id| *id)
    }

    pub fn get_opposite_lane(&self, lane: LaneID, map: &Map) -> Option<LaneID> {
        let lane_type = map.get_l(lane).lane_type;
        let forwards: Vec<LaneID> = self.children_forwards
            .iter()
            .filter(|&&id| map.get_l(id).lane_type == lane_type)
            .map(|id| *id)
            .collect();
        let backwards: Vec<LaneID> = self.children_backwards
            .iter()
            .filter(|&&id| map.get_l(id).lane_type == lane_type)
            .map(|id| *id)
            .collect();

        if let Some(idx) = forwards.iter().position(|id| *id == lane) {
            return backwards.get(idx).map(|id| *id);
        }
        if let Some(idx) = backwards.iter().position(|id| *id == lane) {
            return forwards.get(idx).map(|id| *id);
        }
        panic!("{} doesn't contain {}", self.id, lane);
    }

    fn get_siblings(&self, lane: LaneID) -> &Vec<LaneID> {
        // TODO rm lane from this list?
        if self.children_forwards.contains(&lane) {
            return &self.children_forwards;
        }
        if self.children_backwards.contains(&lane) {
            return &self.children_backwards;
        }
        panic!("{} doesn't contain {}", self.id, lane);
    }
}
