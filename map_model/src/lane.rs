use crate::pathfind;
use crate::{
    osm, BuildingID, BusStopID, DirectedRoadID, IntersectionID, Map, PathConstraints, Road, RoadID,
    TurnType,
};
use geom::{Angle, Distance, Line, PolyLine, Pt2D};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

// Bit longer than the longest car.
pub const PARKING_SPOT_LENGTH: Distance = Distance::const_meters(8.0);

// TODO reconsider pub usize. maybe outside world shouldnt know.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct LaneID(pub usize);

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
    Biking,
    Bus,
    SharedLeftTurn,
    Construction,
}

impl LaneType {
    pub fn is_for_moving_vehicles(self) -> bool {
        match self {
            LaneType::Driving => true,
            LaneType::Biking => true,
            LaneType::Bus => true,
            LaneType::Parking => false,
            LaneType::Sidewalk => false,
            LaneType::SharedLeftTurn => false,
            LaneType::Construction => false,
        }
    }

    pub fn supports_any_movement(self) -> bool {
        match self {
            LaneType::Driving => true,
            LaneType::Biking => true,
            LaneType::Bus => true,
            LaneType::Parking => false,
            LaneType::Sidewalk => true,
            LaneType::SharedLeftTurn => false,
            LaneType::Construction => false,
        }
    }

    pub fn describe(self) -> &'static str {
        match self {
            LaneType::Driving => "a general-purpose driving lane",
            LaneType::Biking => "a protected bike lane",
            LaneType::Bus => "a bus-only lane",
            LaneType::Parking => "an on-street parking lane",
            LaneType::Sidewalk => "a sidewalk",
            LaneType::SharedLeftTurn => "a shared left-turn lane",
            LaneType::Construction => "a lane that's closed for construction",
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

    // Sorted by distance of the front path
    pub building_paths: Vec<BuildingID>,
    pub bus_stops: Vec<BusStopID>,

    // If set, cars trying to park near here should actually start their search at this other lane.
    // Only populated for driving lanes inevitably leading to borders.
    pub parking_blackhole: Option<LaneID>,
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

    pub fn dist_along(&self, dist_along: Distance) -> (Pt2D, Angle) {
        self.lane_center_pts.dist_along(dist_along)
    }

    pub fn safe_dist_along(&self, dist_along: Distance) -> Option<(Pt2D, Angle)> {
        self.lane_center_pts.safe_dist_along(dist_along)
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

    pub fn is_sidewalk(&self) -> bool {
        self.lane_type == LaneType::Sidewalk
    }

    pub fn is_parking(&self) -> bool {
        self.lane_type == LaneType::Parking
    }

    // TODO Store this natively if this winds up being useful.
    pub fn get_directed_parent(&self, map: &Map) -> DirectedRoadID {
        let r = map.get_r(self.parent);
        if r.is_forwards(self.id) {
            r.id.forwards()
        } else {
            r.id.backwards()
        }
    }

    pub fn get_turn_restrictions(&self, road: &Road) -> Option<BTreeSet<TurnType>> {
        if !self.is_driving() {
            return None;
        }

        let (dir, offset) = road.dir_and_offset(self.id);
        let all = if dir && road.osm_tags.contains_key(osm::ENDPT_FWD) {
            road.osm_tags
                .get("turn:lanes:forward")
                .or_else(|| road.osm_tags.get("turn:lanes"))?
        } else if !dir && road.osm_tags.contains_key(osm::ENDPT_BACK) {
            road.osm_tags.get("turn:lanes:backward")?
        } else {
            return None;
        };
        let parts: Vec<&str> = all.split('|').collect();
        // TODO Verify the number of lanes matches up
        let part = parts.get(offset)?;
        if part == &"none" {
            return None;
        }
        Some(
            part.split(';')
                .flat_map(|s| match s {
                    "left" | "left\\left" => vec![TurnType::Left],
                    "right" => vec![TurnType::Right],
                    // TODO What is blank supposed to mean? From few observed cases, same as through
                    "through" | "" => vec![
                        TurnType::Straight,
                        TurnType::LaneChangeLeft,
                        TurnType::LaneChangeRight,
                    ],
                    // TODO Check this more carefully
                    "slight_right" | "slight right" | "merge_to_right" => vec![
                        TurnType::Straight,
                        TurnType::LaneChangeRight,
                        TurnType::Right,
                    ],
                    "slight_left" | "slight left" | "merge_to_left" | "sharp_left" => {
                        vec![TurnType::Straight, TurnType::LaneChangeLeft, TurnType::Left]
                    }
                    "reverse" => {
                        // TODO We need TurnType::UTurn. Until then, u-turns usually show up as
                        // left turns.
                        vec![TurnType::Left]
                    }
                    _ => panic!("What's turn restriction {}?", s),
                })
                .collect(),
        )
    }

    pub fn get_max_cost(&self, constraints: PathConstraints, map: &Map) -> usize {
        map.get_turns_to_lane(self.id)
            .into_iter()
            .map(|turn| pathfind::cost(self, turn, constraints, map))
            .max()
            .unwrap_or_else(|| {
                // Probably a border.
                println!("{} has no incoming turns! Bogus cost 0", self.id);
                0
            })
    }

    // Returns [-1.0, 1.0]. 0 is flat, positive is uphill, negative is downhill.
    // TODO Or do we care about the total up/down along the possibly long road?
    pub fn percent_grade(&self, map: &Map) -> f64 {
        let rise = map.get_i(self.dst_i).elevation - map.get_i(self.src_i).elevation;
        let run = self.length();
        let grade = rise / run;
        if grade <= -1.0 || grade >= 1.0 {
            // TODO Panic
            println!("Grade of {} is {}%", self.id, grade * 100.0);
            if grade < 0.0 {
                return -1.0;
            } else {
                return 1.0;
            }
        }
        grade
    }
}
