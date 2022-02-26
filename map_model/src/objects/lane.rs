use std::collections::BTreeSet;
use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use geom::{Distance, Line, PolyLine, Polygon, Pt2D};
use raw_map::LaneType;

use crate::{
    osm, DirectedRoadID, Direction, DrivingSide, IntersectionID, Map, MapConfig, Road, RoadID,
    RoadSideID, SideOfRoad, TurnType,
};

/// From some manually audited cases in Seattle, the length of parallel street parking spots is a
/// bit different than the length in parking lots, so set a different value here.
pub const PARKING_LOT_SPOT_LENGTH: Distance = Distance::const_meters(6.4);

/// A lane is identified by its parent road and its position, ordered from the left.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord)]
pub struct LaneID {
    pub road: RoadID,
    pub offset: usize,
}

impl fmt::Display for LaneID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Lane #{}", self.encode_u32())
    }
}

impl LaneID {
    // TODO Do we have an endianness problem, or does serde take care of us?
    pub fn encode_u32(self) -> u32 {
        // The first 27 bits encode the RoadID, the last 5 the offset.
        //
        // (In some Houston area dystopia, we might want 2^5 = 32 lanes on one road. That leaves 27
        // bits for roads -- 134 million roads should be plenty.)
        let road = self.road.0 << 5;
        (road | self.offset) as u32
    }

    pub fn decode_u32(x: u32) -> LaneID {
        let road = RoadID((x >> 5) as usize);
        // My bit twiddling is weak. Easier way to get a binary mask starting with 11111?
        let offset = (x & (1 + 2 + 4 + 8 + 16)) as usize;
        LaneID { road, offset }
    }

    pub fn dummy() -> LaneID {
        LaneID {
            road: RoadID(0),
            offset: 0,
        }
    }
}

impl Serialize for LaneID {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.encode_u32().serialize(s)
    }
}

impl<'de> Deserialize<'de> for LaneID {
    fn deserialize<D>(d: D) -> Result<LaneID, D::Error>
    where
        D: Deserializer<'de>,
    {
        let x = <u32>::deserialize(d)?;
        Ok(LaneID::decode_u32(x))
    }
}

/// A road segment is broken down into individual lanes, which have a LaneType.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Lane {
    pub id: LaneID,
    pub lane_type: LaneType,
    pub lane_center_pts: PolyLine,
    pub width: Distance,
    pub dir: Direction,

    pub src_i: IntersectionID,
    pub dst_i: IntersectionID,

    /// {Cars, bikes} trying to start or end here might not be able to reach most lanes in the
    /// graph, because this is near a border.
    pub driving_blackhole: bool,
    pub biking_blackhole: bool,
}

impl Lane {
    // TODO most of these are wrappers; stop doing this?
    pub fn first_pt(&self) -> Pt2D {
        self.lane_center_pts.first_pt()
    }
    pub fn last_pt(&self) -> Pt2D {
        self.lane_center_pts.last_pt()
    }
    pub fn first_line(&self) -> Line {
        self.lane_center_pts.first_line()
    }
    pub fn last_line(&self) -> Line {
        self.lane_center_pts.last_line()
    }

    pub fn endpoint(&self, i: IntersectionID) -> Pt2D {
        if i == self.src_i {
            self.first_pt()
        } else if i == self.dst_i {
            self.last_pt()
        } else {
            panic!("{} isn't an endpoint of {}", i, self.id);
        }
    }

    /// pt2 will be endpoint
    pub fn end_line(&self, i: IntersectionID) -> Line {
        if i == self.src_i {
            self.first_line().reversed()
        } else if i == self.dst_i {
            self.last_line()
        } else {
            panic!("{} isn't an endpoint of {}", i, self.id);
        }
    }

    pub fn dist_along_of_point(&self, pt: Pt2D) -> Option<Distance> {
        self.lane_center_pts
            .dist_along_of_point(pt)
            .map(|(dist, _)| dist)
    }

    pub fn length(&self) -> Distance {
        self.lane_center_pts.length()
    }

    pub fn intersections(&self) -> Vec<IntersectionID> {
        // TODO I think we're assuming there are no loop lanes
        vec![self.src_i, self.dst_i]
    }

    // TODO different types for each lane type might be reasonable

    pub fn number_parking_spots(&self, cfg: &MapConfig) -> usize {
        assert_eq!(self.lane_type, LaneType::Parking);
        // No spots next to intersections
        let spots = (self.length() / cfg.street_parking_spot_length).floor() - 2.0;
        if spots >= 1.0 {
            spots as usize
        } else {
            0
        }
    }

    pub fn is_driving(&self) -> bool {
        self.lane_type == LaneType::Driving
    }

    pub fn is_biking(&self) -> bool {
        self.lane_type == LaneType::Biking
    }

    pub fn is_bus(&self) -> bool {
        self.lane_type == LaneType::Bus
    }

    pub fn is_walkable(&self) -> bool {
        self.lane_type.is_walkable()
    }

    pub fn is_sidewalk(&self) -> bool {
        self.lane_type == LaneType::Sidewalk
    }

    pub fn is_shoulder(&self) -> bool {
        self.lane_type == LaneType::Shoulder
    }

    pub fn is_parking(&self) -> bool {
        self.lane_type == LaneType::Parking
    }

    pub fn is_light_rail(&self) -> bool {
        self.lane_type == LaneType::LightRail
    }

    pub fn get_directed_parent(&self) -> DirectedRoadID {
        DirectedRoadID {
            road: self.id.road,
            dir: self.dir,
        }
    }

    /// This does the reasonable thing for the leftmost and rightmost lane on a road -- except for
    /// roads with exactly one lane. For lanes in the middle of a road, it uses the direction of
    /// the lane -- so bidirectional/contraflow cycletracks will produce weird results.
    // TODO This is such a weird API; make blockfinding not depend on this
    pub fn get_nearest_side_of_road(&self, map: &Map) -> RoadSideID {
        if self.id.offset == 0 {
            return RoadSideID {
                road: self.id.road,
                side: SideOfRoad::Left,
            };
        }
        let parent = map.get_r(self.id.road);
        if parent.lanes.last().as_ref().unwrap().id == self.id {
            return RoadSideID {
                road: self.id.road,
                side: SideOfRoad::Right,
            };
        }

        let side = match (self.dir, map.get_config().driving_side) {
            (Direction::Fwd, DrivingSide::Right) => SideOfRoad::Right,
            (Direction::Back, DrivingSide::Right) => SideOfRoad::Left,
            (Direction::Fwd, DrivingSide::Left) => SideOfRoad::Left,
            (Direction::Back, DrivingSide::Left) => SideOfRoad::Right,
        };
        RoadSideID {
            road: self.id.road,
            side,
        }
    }

    /// Returns the set of allowed turn types, based on individual turn lane restrictions. `None`
    /// means all turn types are allowed.
    ///
    /// This will return `None` for bus lanes, unless `force_bus` is true. OSM turn restrictions on
    /// bus lanes usually apply to regular vehicles, not the buses. When generating the turns for
    /// buses, we probably don't want to use the restrictions.
    pub fn get_lane_level_turn_restrictions(
        &self,
        road: &Road,
        force_bus: bool,
    ) -> Option<BTreeSet<TurnType>> {
        if !self.is_driving() && (!force_bus || !self.is_bus()) {
            return None;
        }

        let all = if self.dir == Direction::Fwd && road.osm_tags.contains_key(osm::ENDPT_FWD) {
            road.osm_tags
                .get("turn:lanes:forward")
                .or_else(|| road.osm_tags.get("turn:lanes"))?
        } else if self.dir == Direction::Back && road.osm_tags.contains_key(osm::ENDPT_BACK) {
            road.osm_tags.get("turn:lanes:backward")?
        } else {
            return None;
        };
        let parts: Vec<&str> = all.split('|').collect();
        // Verify the number of parts matches the road's lanes
        let lanes: Vec<LaneID> = road
            .children(self.dir)
            .into_iter()
            .filter(|(_, lt)| *lt == LaneType::Driving || *lt == LaneType::Bus)
            .map(|(id, _)| id)
            .collect();
        if parts.len() != lanes.len() {
            warn!("{}'s turn restrictions don't match the lanes", road.orig_id);
            return None;
        }
        // TODO More warnings if this fails
        let part = parts[lanes.iter().position(|l| *l == self.id)?];

        // TODO Probably the target lane should get marked as LaneType::Bus
        if part == "yes" || part == "psv" || part == "bus" {
            return None;
        }

        // These both mean that physically, there's no marking saying what turn is valid. In
        // practice, this seems to imply straight is always fine, and right/left are fine unless
        // covered by an explicit turn lane.
        //
        // If a multi-lane road lacks markings, just listening to this function will mean that the
        // rightmos lanes could turn left, which probably isn't great for people in the middle
        // lanes going straight. Further filtering (in remove_merging_turns) will prune this out.
        if part.is_empty() || part == "none" {
            let all_explicit_types: BTreeSet<TurnType> = parts
                .iter()
                .flat_map(|part| part.split(';').flat_map(parse_turn_type_from_osm))
                .collect();
            let mut implied = BTreeSet::new();
            implied.insert(TurnType::Straight);
            for tt in [TurnType::Left, TurnType::Right] {
                if !all_explicit_types.contains(&tt) {
                    implied.insert(tt);
                }
            }
            return Some(implied);
        }

        Some(part.split(';').flat_map(parse_turn_type_from_osm).collect())
    }

    pub fn common_endpoint(&self, other: &Lane) -> CommonEndpoint {
        CommonEndpoint::new((self.src_i, self.dst_i), (other.src_i, other.dst_i))
    }

    pub fn get_thick_polygon(&self) -> Polygon {
        self.lane_center_pts.make_polygons(self.width)
    }
}

#[derive(PartialEq)]
pub enum CommonEndpoint {
    /// Two lanes/roads share one endpoint
    One(IntersectionID),
    /// Two lanes/roads share both endpoints, because they both belong to the same road, or there
    /// are two different roads connecting the same pair of intersections
    Both,
    /// Two lanes/roads don't have any common endpoints
    None,
}

impl CommonEndpoint {
    pub fn new(
        obj1: (IntersectionID, IntersectionID),
        obj2: (IntersectionID, IntersectionID),
    ) -> CommonEndpoint {
        #![allow(clippy::suspicious_operation_groupings)]
        let src = obj1.0 == obj2.0 || obj1.0 == obj2.1;
        let dst = obj1.1 == obj2.0 || obj1.1 == obj2.1;
        if src && dst {
            return CommonEndpoint::Both;
        }
        if src {
            return CommonEndpoint::One(obj1.0);
        }
        if dst {
            return CommonEndpoint::One(obj1.1);
        }
        CommonEndpoint::None
    }
}

// See https://wiki.openstreetmap.org/wiki/Key:turn
fn parse_turn_type_from_osm(x: &str) -> Vec<TurnType> {
    match x {
        "left" => vec![TurnType::Left],
        "right" => vec![TurnType::Right],
        "through" => vec![TurnType::Straight],
        "slight_right" | "slight right" | "merge_to_right" | "sharp_right" => {
            vec![TurnType::Straight, TurnType::Right]
        }
        "slight_left" | "slight left" | "merge_to_left" | "sharp_left" => {
            vec![TurnType::Straight, TurnType::Left]
        }
        "reverse" => vec![TurnType::UTurn],
        "none" | "" => vec![],
        _ => {
            warn!("Unknown turn restriction {}", x);
            vec![]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lane_id_encoding() {
        let l = LaneID {
            road: RoadID(42),
            offset: 3,
        };
        assert_eq!(l, LaneID::decode_u32(l.encode_u32()));
    }
}
