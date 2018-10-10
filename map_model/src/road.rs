use abstutil::Error;
use dimensioned::si;
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
#[derive(Serialize, Deserialize, Debug)]
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
        if let Some(idx) = self
            .children_forwards
            .iter()
            .position(|pair| pair.0 == lane)
        {
            return (true, idx);
        }
        if let Some(idx) = self
            .children_backwards
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

    pub fn find_sidewalk(&self, parking_or_driving: LaneID) -> Result<LaneID, Error> {
        self.get_siblings(parking_or_driving)
            .iter()
            .find(|pair| pair.1 == LaneType::Sidewalk)
            .map(|pair| pair.0)
            .ok_or_else(|| {
                Error::new(format!(
                    "{} doesn't have sidewalk sibling",
                    parking_or_driving
                ))
            })
    }

    pub fn find_driving_lane(&self, parking: LaneID) -> Result<LaneID, Error> {
        //assert_eq!(l.lane_type, LaneType::Parking);
        self.get_siblings(parking)
            .iter()
            .find(|pair| pair.1 == LaneType::Driving)
            .map(|pair| pair.0)
            .ok_or_else(|| Error::new(format!("{} doesn't have driving lane sibling", parking)))
    }

    // Handles intermediate parking and bus lanes and such
    // Additionally handles one-ways with a sidewalk on only one side.
    // TODO but in reality, there probably isn't a sidewalk on the other side of the one-way. :\
    pub fn find_driving_lane_from_sidewalk(&self, sidewalk: LaneID) -> Result<LaneID, Error> {
        let (this_side, opposite, idx) = if let Some(idx) = self
            .children_forwards
            .iter()
            .position(|(l, _)| *l == sidewalk)
        {
            (&self.children_forwards, &self.children_backwards, idx)
        } else if let Some(idx) = self
            .children_backwards
            .iter()
            .position(|(l, _)| *l == sidewalk)
        {
            (&self.children_backwards, &self.children_forwards, idx)
        } else {
            panic!("{} doesn't contain {}", self.id, sidewalk)
        };
        // Sidewalks are always at the end
        assert!(idx == this_side.len() - 1);
        // So is there a driving lane on this side?
        if let Some(l) = this_side
            .iter()
            .rev()
            .find(|(_, lt)| *lt == LaneType::Driving)
            .map(|(l, _)| *l)
        {
            return Ok(l);
        }

        // Is the sidewalk on a one-way with the other side having a driving lane?
        if this_side.len() == 1 && opposite[0].1 == LaneType::Driving {
            return Ok(opposite[0].0);
        }
        Err(Error::new(format!(
            "Sidewalk {} doesn't have driving lane",
            sidewalk
        )))
    }

    pub fn find_parking_lane(&self, driving: LaneID) -> Result<LaneID, Error> {
        //assert_eq!(l.lane_type, LaneType::Driving);
        self.get_siblings(driving)
            .iter()
            .find(|pair| pair.1 == LaneType::Parking)
            .map(|pair| pair.0)
            .ok_or_else(|| Error::new(format!("{} doesn't have parking lane sibling", driving)))
    }

    pub fn get_opposite_lane(&self, lane: LaneID, lane_type: LaneType) -> Result<LaneID, Error> {
        let forwards: Vec<LaneID> = self
            .children_forwards
            .iter()
            .filter(|pair| pair.1 == lane_type)
            .map(|pair| pair.0)
            .collect();
        let backwards: Vec<LaneID> = self
            .children_backwards
            .iter()
            .filter(|pair| pair.1 == lane_type)
            .map(|pair| pair.0)
            .collect();

        if let Some(idx) = forwards.iter().position(|id| *id == lane) {
            return backwards.get(idx).map(|id| *id).ok_or_else(|| {
                Error::new(format!(
                    "{} doesn't have opposite lane of type {:?}",
                    lane, lane_type
                ))
            });
        }
        if let Some(idx) = backwards.iter().position(|id| *id == lane) {
            return forwards.get(idx).map(|id| *id).ok_or_else(|| {
                Error::new(format!(
                    "{} doesn't have opposite lane of type {:?}",
                    lane, lane_type
                ))
            });
        }
        panic!("{} doesn't contain {}", self.id, lane);
    }

    // Only the immediately adjacent siblings -- so could be 0, 1, or 2 results.
    fn get_siblings(&self, lane: LaneID) -> Vec<(LaneID, LaneType)> {
        let (list, idx) = if let Some(idx) = self
            .children_forwards
            .iter()
            .position(|pair| pair.0 == lane)
        {
            (&self.children_forwards, idx)
        } else if let Some(idx) = self
            .children_backwards
            .iter()
            .position(|pair| pair.0 == lane)
        {
            (&self.children_backwards, idx)
        } else {
            panic!("{} doesn't contain {}", self.id, lane)
        };

        let mut result = Vec::new();
        if idx != 0 {
            result.push(list[idx - 1]);
        }
        if idx != list.len() - 1 {
            result.push(list[idx + 1]);
        }
        result
    }

    pub fn get_speed_limit(&self) -> si::MeterPerSecond<f64> {
        if self.osm_tags.get("highway") == Some(&"primary".to_string())
            || self.osm_tags.get("highway") == Some(&"secondary".to_string())
        {
            // 40mph
            return 17.8816 * si::MPS;
        }
        // 20mph
        8.9408 * si::MPS
    }
}
