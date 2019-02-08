use crate::{raw_data, IntersectionID, LaneID, LaneType};
use abstutil::Error;
use geom::{PolyLine, Speed};
use serde_derive::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::fmt;

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
    pub stable_id: raw_data::StableRoadID,

    // Invariant: A road must contain at least one child
    pub children_forwards: Vec<(LaneID, LaneType)>,
    pub children_backwards: Vec<(LaneID, LaneType)>,
    // TODO should consider having a redundant lookup from LaneID

    // Unshifted center points. Order implies road orientation.
    pub center_pts: PolyLine,
    pub src_i: IntersectionID,
    pub dst_i: IntersectionID,

    // For debugging.
    pub original_center_pts: PolyLine,
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

    pub fn is_forwards(&self, lane: LaneID) -> bool {
        self.children_forwards.iter().any(|(id, _)| *id == lane)
    }

    pub fn is_backwards(&self, lane: LaneID) -> bool {
        self.children_backwards.iter().any(|(id, _)| *id == lane)
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

    pub fn get_speed_limit(&self) -> Speed {
        // TODO Should probably cache this
        if let Some(limit) = self.osm_tags.get("maxspeed") {
            // TODO handle other units
            if limit.ends_with(" mph") {
                if let Ok(mph) = limit[0..limit.len() - 4].parse::<f64>() {
                    return Speed::miles_per_hour(mph);
                }
            }
        }

        if self.osm_tags.get("highway") == Some(&"primary".to_string())
            || self.osm_tags.get("highway") == Some(&"secondary".to_string())
        {
            return Speed::miles_per_hour(40.0);
        }
        Speed::miles_per_hour(20.0)
    }

    pub fn get_zorder(&self) -> isize {
        // TODO Should probably cache this
        if let Some(layer) = self.osm_tags.get("layer") {
            layer.parse::<isize>().unwrap()
        } else {
            0
        }
    }

    pub fn incoming_lanes(&self, i: IntersectionID) -> &Vec<(LaneID, LaneType)> {
        if self.src_i == i {
            &self.children_backwards
        } else if self.dst_i == i {
            &self.children_forwards
        } else {
            panic!("{} doesn't have an endpoint at {}", self.id, i);
        }
    }

    pub fn outgoing_lanes(&self, i: IntersectionID) -> &Vec<(LaneID, LaneType)> {
        if self.src_i == i {
            &self.children_forwards
        } else if self.dst_i == i {
            &self.children_backwards
        } else {
            panic!("{} doesn't have an endpoint at {}", self.id, i);
        }
    }

    // If 'from' is a sidewalk, we'll also consider lanes on the other side of the road, if needed.
    // TODO But reusing dist_along will break loudly in that case! Really need a perpendicular
    // projection-and-collision method to find equivalent dist_along's.
    pub(crate) fn find_closest_lane(
        &self,
        from: LaneID,
        types: Vec<LaneType>,
    ) -> Result<LaneID, Error> {
        let lane_types: HashSet<LaneType> = types.into_iter().collect();
        let (dir, from_idx) = self.dir_and_offset(from);
        let mut list = if dir {
            &self.children_forwards
        } else {
            &self.children_backwards
        };
        // Deal with one-ways and sidewalks on both sides
        if list.len() == 1 && list[0].1 == LaneType::Sidewalk {
            list = if dir {
                &self.children_backwards
            } else {
                &self.children_forwards
            };
        }

        if let Some((_, lane)) = list
            .iter()
            .enumerate()
            .filter(|(_, (lane, lt))| *lane != from && lane_types.contains(lt))
            .map(|(idx, (lane, _))| (((from_idx as isize) - (idx as isize)).abs(), *lane))
            .min_by_key(|(offset, _)| *offset)
        {
            Ok(lane)
        } else {
            Err(Error::new(format!(
                "{} isn't near a {:?} lane",
                from, lane_types
            )))
        }
    }

    pub fn supports_bikes(&self) -> bool {
        // TODO Should check LaneType to start
        self.osm_tags.get("bicycle") != Some(&"no".to_string())
    }
}
