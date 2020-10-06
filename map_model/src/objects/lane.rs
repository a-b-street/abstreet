// A road segment is broken down into individual lanes, which have a LaneType.

use std::collections::BTreeSet;
use std::fmt;

use serde::{Deserialize, Serialize};

use abstutil::{deserialize_usize, serialize_usize};
use geom::{Distance, Line, PolyLine, Pt2D};

use crate::{
    osm, BusStopID, DirectedRoadID, Direction, IntersectionID, Map, Road, RoadID, TurnType,
};

// Bit longer than the longest car.
pub const PARKING_SPOT_LENGTH: Distance = Distance::const_meters(8.0);
// The full PARKING_SPOT_LENGTH used for on-street is looking too conservative for some manually
// audited cases in Seattle. This is 0.8 of above
pub const PARKING_LOT_SPOT_LENGTH: Distance = Distance::const_meters(6.4);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct LaneID(
    #[serde(
        serialize_with = "serialize_usize",
        deserialize_with = "deserialize_usize"
    )]
    pub usize,
);

impl fmt::Display for LaneID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Lane #{}", self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum LaneType {
    Driving,
    Parking,
    Sidewalk,
    // Walkable like a Sidewalk, but very narrow. Used to model pedestrians walking on roads
    // without sidewalks.
    Shoulder,
    Biking,
    Bus,
    SharedLeftTurn,
    Construction,
    LightRail,
}

impl LaneType {
    pub fn is_for_moving_vehicles(self) -> bool {
        match self {
            LaneType::Driving => true,
            LaneType::Biking => true,
            LaneType::Bus => true,
            LaneType::Parking => false,
            LaneType::Sidewalk => false,
            LaneType::Shoulder => false,
            LaneType::SharedLeftTurn => false,
            LaneType::Construction => false,
            LaneType::LightRail => true,
        }
    }

    pub fn supports_any_movement(self) -> bool {
        match self {
            LaneType::Driving => true,
            LaneType::Biking => true,
            LaneType::Bus => true,
            LaneType::Parking => false,
            LaneType::Sidewalk => true,
            LaneType::Shoulder => true,
            LaneType::SharedLeftTurn => false,
            LaneType::Construction => false,
            LaneType::LightRail => true,
        }
    }

    pub fn describe(self) -> &'static str {
        match self {
            LaneType::Driving => "a general-purpose driving lane",
            LaneType::Biking => "a protected bike lane",
            LaneType::Bus => "a bus-only lane",
            LaneType::Parking => "an on-street parking lane",
            LaneType::Sidewalk => "a sidewalk",
            LaneType::Shoulder => "a shoulder",
            LaneType::SharedLeftTurn => "a shared left-turn lane",
            LaneType::Construction => "a lane that's closed for construction",
            LaneType::LightRail => "a light rail track",
        }
    }

    pub fn short_name(self) -> &'static str {
        match self {
            LaneType::Driving => "driving lane",
            LaneType::Biking => "bike lane",
            LaneType::Bus => "bus lane",
            LaneType::Parking => "parking lane",
            LaneType::Sidewalk => "sidewalk",
            LaneType::Shoulder => "shoulder",
            LaneType::SharedLeftTurn => "left-turn lane",
            LaneType::Construction => "construction",
            LaneType::LightRail => "light rail track",
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Lane {
    pub id: LaneID,
    pub parent: RoadID,
    pub lane_type: LaneType,
    pub lane_center_pts: PolyLine,
    pub width: Distance,

    pub src_i: IntersectionID,
    pub dst_i: IntersectionID,

    // Meaningless order
    pub bus_stops: BTreeSet<BusStopID>,

    // {Cars, bikes} trying to start or end here might not be able to reach most lanes in the
    // graph, because this is near a border.
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

    // pt2 will be endpoint
    pub fn end_line(&self, i: IntersectionID) -> Line {
        if i == self.src_i {
            self.first_line().reverse()
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

    pub fn number_parking_spots(&self) -> usize {
        assert_eq!(self.lane_type, LaneType::Parking);
        // No spots next to intersections
        let spots = (self.length() / PARKING_SPOT_LENGTH).floor() - 2.0;
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
        self.lane_type == LaneType::Sidewalk || self.lane_type == LaneType::Shoulder
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

    // TODO Store this natively if this winds up being useful.
    pub(crate) fn get_directed_parent(&self, map: &Map) -> DirectedRoadID {
        let r = map.get_r(self.parent);
        DirectedRoadID {
            id: r.id,
            dir: r.dir(self.id),
        }
    }

    pub fn get_turn_restrictions(&self, road: &Road) -> Option<BTreeSet<TurnType>> {
        if !self.is_driving() {
            return None;
        }

        let dir = road.dir(self.id);
        let all = if dir == Direction::Fwd && road.osm_tags.contains_key(osm::ENDPT_FWD) {
            road.osm_tags
                .get("turn:lanes:forward")
                .or_else(|| road.osm_tags.get("turn:lanes"))?
        } else if dir == Direction::Back && road.osm_tags.contains_key(osm::ENDPT_BACK) {
            road.osm_tags.get("turn:lanes:backward")?
        } else {
            return None;
        };
        let parts: Vec<&str> = all.split('|').collect();
        // Verify the number of parts matches the road's lanes
        let lanes: Vec<LaneID> = road
            .children(dir)
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
        if part == "no" || part == "none" || part == "yes" || part == "psv" || part == "bus" {
            return None;
        }
        // Empty means no restrictions
        if part == "" {
            return None;
        }
        Some(
            part.split(';')
                .flat_map(|s| match s {
                    "left" | "left\\left" => vec![TurnType::Left],
                    "right" => vec![TurnType::Right],
                    // TODO What is blank supposed to mean? From few observed cases, same as through
                    "through" | "" => vec![TurnType::Straight],
                    // TODO Check this more carefully
                    "slight_right" | "slight right" | "merge_to_right" | "sharp_right" => {
                        vec![TurnType::Straight, TurnType::Right]
                    }
                    "slight_left" | "slight left" | "merge_to_left" | "sharp_left" => {
                        vec![TurnType::Straight, TurnType::Left]
                    }
                    "reverse" => {
                        // TODO We need TurnType::UTurn. Until then, u-turns usually show up as
                        // left turns.
                        vec![TurnType::Left]
                    }
                    s => {
                        warn!("Unknown turn restriction {}", s);
                        vec![]
                    }
                })
                .collect(),
        )
    }
}
