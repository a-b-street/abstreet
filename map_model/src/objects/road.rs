use std::fmt;

use enumset::EnumSet;
use serde::{Deserialize, Serialize};

use abstutil::{deserialize_usize, serialize_usize, Tags};
use geom::{Distance, PolyLine, Polygon, Speed};

use crate::raw::{OriginalRoad, RestrictionType};
use crate::{
    osm, AccessRestrictions, BusStopID, IntersectionID, Lane, LaneID, LaneType, Map,
    PathConstraints, Zone,
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RoadID(
    #[serde(
        serialize_with = "serialize_usize",
        deserialize_with = "deserialize_usize"
    )]
    pub usize,
);

impl fmt::Display for RoadID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Road #{}", self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Direction {
    Fwd,
    Back,
}

impl Direction {
    pub fn opposite(self) -> Direction {
        match self {
            Direction::Fwd => Direction::Back,
            Direction::Back => Direction::Fwd,
        }
    }
}

impl fmt::Display for Direction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Direction::Fwd => write!(f, "forwards"),
            Direction::Back => write!(f, "backwards"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct DirectedRoadID {
    pub id: RoadID,
    pub dir: Direction,
}

impl fmt::Display for DirectedRoadID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "DirectedRoadID({}, {})", self.id.0, self.dir,)
    }
}

impl DirectedRoadID {
    pub fn src_i(self, map: &Map) -> IntersectionID {
        let r = map.get_r(self.id);
        if self.dir == Direction::Fwd {
            r.src_i
        } else {
            r.dst_i
        }
    }

    pub fn dst_i(self, map: &Map) -> IntersectionID {
        let r = map.get_r(self.id);
        if self.dir == Direction::Fwd {
            r.dst_i
        } else {
            r.src_i
        }
    }

    /// Strict for bikes. If there are bike lanes, not allowed to use other lanes.
    pub fn lanes(self, constraints: PathConstraints, map: &Map) -> Vec<LaneID> {
        let r = map.get_r(self.id);
        constraints.filter_lanes(r.children(self.dir).iter().map(|(l, _)| *l).collect(), map)
    }
}

/// A Road represents a segment between exactly two Intersections. It contains Lanes as children.
#[derive(Serialize, Deserialize, Debug)]
pub struct Road {
    pub id: RoadID,
    pub osm_tags: Tags,
    /// self is 'from'
    pub turn_restrictions: Vec<(RestrictionType, RoadID)>,
    /// self is 'from'. (via, to). Only BanTurns.
    pub complicated_turn_restrictions: Vec<(RoadID, RoadID)>,
    pub orig_id: OriginalRoad,
    pub speed_limit: Speed,
    pub access_restrictions: AccessRestrictions,
    pub zorder: isize,

    /// Invariant: A road must contain at least one child
    // TODO Only public for Map::import_minimal. Can we avoid this?
    pub lanes_ltr: Vec<(LaneID, Direction, LaneType)>,

    /// The physical center of the road, including sidewalks, after trimming. The order implies
    /// road orientation. No edits ever change this.
    // TODO Maybe deprecated in favor of get_left_side?
    pub center_pts: PolyLine,
    pub src_i: IntersectionID,
    pub dst_i: IntersectionID,
}

impl Road {
    /// Returns all lanes from the left side of the road to right. Left/right is determined by the
    /// orientation of center_pts.
    pub fn lanes_ltr(&self) -> Vec<(LaneID, Direction, LaneType)> {
        // TODO Change this to return &Vec
        self.lanes_ltr.clone()
    }

    pub fn get_left_side(&self, map: &Map) -> PolyLine {
        self.center_pts.must_shift_left(self.get_half_width(map))
    }

    /// Counting from the left side of the road
    pub fn offset(&self, lane: LaneID) -> usize {
        for (idx, (l, _, _)) in self.lanes_ltr().into_iter().enumerate() {
            if lane == l {
                return idx;
            }
        }
        panic!("{} doesn't contain {}", self.id, lane);
    }

    pub fn dir(&self, lane: LaneID) -> Direction {
        for (l, dir, _) in self.lanes_ltr() {
            if lane == l {
                return dir;
            }
        }
        panic!("{} doesn't contain {}", self.id, lane);
    }

    pub fn parking_to_driving(&self, parking: LaneID, map: &Map) -> Option<LaneID> {
        self.find_closest_lane(parking, |l| l.is_driving(), map)
    }

    pub(crate) fn speed_limit_from_osm(&self) -> Speed {
        if let Some(limit) = self.osm_tags.get(osm::MAXSPEED) {
            if let Ok(kmph) = limit.parse::<f64>() {
                return Speed::km_per_hour(kmph);
            }

            if let Some(mph) = limit
                .strip_suffix(" mph")
                .and_then(|x| x.parse::<f64>().ok())
            {
                return Speed::miles_per_hour(mph);
            }

            // TODO Handle implicits, like PL:zone30
        }

        // These're half reasonable guesses. Better to explicitly tag in OSM.
        if self
            .osm_tags
            .is_any(osm::HIGHWAY, vec!["primary", "secondary", "motorway_link"])
        {
            return Speed::miles_per_hour(40.0);
        }
        if self.osm_tags.is(osm::HIGHWAY, "living_street") {
            // about 12mph
            return Speed::km_per_hour(20.0);
        }
        if self.is_service() {
            return Speed::miles_per_hour(10.0);
        }
        Speed::miles_per_hour(20.0)
    }

    /// Includes off-side
    // TODO Specialize a variant for PathConstraints.can_use. Only one caller needs something
    // fancier.
    pub fn find_closest_lane<F: Fn(&Lane) -> bool>(
        &self,
        from: LaneID,
        filter: F,
        map: &Map,
    ) -> Option<LaneID> {
        let our_idx = self.offset(from) as isize;
        self.lanes_ltr()
            .into_iter()
            .enumerate()
            .filter_map(|(idx, (l, _, _))| {
                if (idx as isize) != our_idx && filter(map.get_l(l)) {
                    Some((idx, l))
                } else {
                    None
                }
            })
            .min_by_key(|(idx, _)| (our_idx - (*idx as isize)).abs())
            .map(|(_, l)| l)
    }

    pub fn all_lanes(&self) -> Vec<LaneID> {
        self.lanes_ltr().into_iter().map(|(l, _, _)| l).collect()
    }

    /// This is the FIRST yellow line where the direction of the road changes. If multiple direction
    /// changes happen, the result is kind of arbitrary.
    pub fn get_dir_change_pl(&self, map: &Map) -> PolyLine {
        let mut found: Option<LaneID> = None;
        for pair in self.lanes_ltr().windows(2) {
            let ((l1, dir1, _), (_, dir2, _)) = (pair[0], pair[1]);
            if dir1 != dir2 {
                found = Some(l1);
                break;
            }
        }
        let lane = map.get_l(found.unwrap_or(self.lanes_ltr()[0].0));
        if self.dir(lane.id) == Direction::Fwd {
            lane.lane_center_pts.must_shift_left(lane.width / 2.0)
        } else {
            lane.lane_center_pts
                .must_shift_left(lane.width / 2.0)
                .reversed()
        }
    }

    pub fn get_half_width(&self, map: &Map) -> Distance {
        self.all_lanes()
            .into_iter()
            .map(|l| map.get_l(l).width)
            .sum::<Distance>()
            / 2.0
    }

    pub fn get_thick_polygon(&self, map: &Map) -> Polygon {
        self.center_pts
            .make_polygons(self.get_half_width(map) * 2.0)
    }

    pub fn get_name(&self, lang: Option<&String>) -> String {
        if let Some(lang) = lang {
            if let Some(name) = self.osm_tags.get(&format!("name:{}", lang)) {
                return name.to_string();
            }
        }

        if let Some(name) = self.osm_tags.get(osm::NAME) {
            if name == "" {
                return "???".to_string();
            } else {
                return name.to_string();
            }
        }
        if let Some(name) = self.osm_tags.get("ref") {
            return name.to_string();
        }
        if self
            .osm_tags
            .get(osm::HIGHWAY)
            .map(|hwy| hwy.ends_with("_link"))
            .unwrap_or(false)
        {
            if let Some(name) = self.osm_tags.get("destination:street") {
                return format!("Exit for {}", name);
            }
            if let Some(name) = self.osm_tags.get("destination:ref") {
                return format!("Exit for {}", name);
            }
            if let Some(name) = self.osm_tags.get("destination") {
                return format!("Exit for {}", name);
            }
            // Sometimes 'directions' is filled out, but incorrectly...
        }
        "???".to_string()
    }

    pub fn get_rank(&self) -> osm::RoadRank {
        if let Some(x) = self.osm_tags.get(osm::HIGHWAY) {
            if x == "construction" {
                // What exactly is under construction?
                if let Some(x) = self.osm_tags.get("construction") {
                    osm::RoadRank::from_highway(x)
                } else {
                    osm::RoadRank::Local
                }
            } else {
                osm::RoadRank::from_highway(x)
            }
        } else {
            osm::RoadRank::Local
        }
    }

    pub fn all_bus_stops(&self, map: &Map) -> Vec<BusStopID> {
        let mut stops = Vec::new();
        for id in self.all_lanes() {
            stops.extend(map.get_l(id).bus_stops.iter().cloned());
        }
        stops
    }

    /// Returns [-1.0, 1.0]. 0 is flat, positive is uphill, negative is downhill.
    // TODO Or do we care about the total up/down along the possibly long road?
    pub fn percent_grade(&self, map: &Map) -> f64 {
        let rise = map.get_i(self.dst_i).elevation - map.get_i(self.src_i).elevation;
        let run = self.center_pts.length();
        let grade = rise / run;
        if grade <= -1.0 || grade >= 1.0 {
            // TODO Panic
            //println!("Grade of {} is {}%", self.id, grade * 100.0);
            if grade < 0.0 {
                return -1.0;
            } else {
                return 1.0;
            }
        }
        grade
    }

    pub fn is_light_rail(&self) -> bool {
        self.lanes_ltr().len() == 1 && self.lanes_ltr()[0].2 == LaneType::LightRail
    }

    pub fn is_footway(&self) -> bool {
        self.lanes_ltr().len() == 1 && self.lanes_ltr()[0].2 == LaneType::Sidewalk
    }

    pub fn is_service(&self) -> bool {
        self.osm_tags.is(osm::HIGHWAY, "service")
    }

    pub fn common_endpt(&self, other: &Road) -> IntersectionID {
        if self.src_i == other.src_i || self.src_i == other.dst_i {
            self.src_i
        } else if self.dst_i == other.src_i || self.dst_i == other.dst_i {
            self.dst_i
        } else {
            panic!("{} and {} don't share an endpoint", self.id, other.id);
        }
    }

    pub fn is_private(&self) -> bool {
        self.access_restrictions != AccessRestrictions::new()
    }

    pub(crate) fn access_restrictions_from_osm(&self) -> AccessRestrictions {
        let allow_through_traffic = if self.osm_tags.is("access", "private") {
            EnumSet::new()
        } else if self.osm_tags.is(osm::HIGHWAY, "living_street") {
            let mut allow = PathConstraints::Pedestrian | PathConstraints::Bike;
            if self.osm_tags.is("psv", "yes") || self.osm_tags.is("bus", "yes") {
                allow |= PathConstraints::Bus;
            }
            allow
        } else {
            EnumSet::all()
        };
        AccessRestrictions {
            allow_through_traffic,
            cap_vehicles_per_hour: None,
        }
    }

    pub fn get_zone<'a>(&self, map: &'a Map) -> Option<&'a Zone> {
        if !self.is_private() {
            return None;
        }
        // Insist on it existing
        Some(
            map.zones
                .iter()
                .find(|z| z.members.contains(&self.id))
                .unwrap(),
        )
    }
}

// TODO All of this is kind of deprecated? During the transiton towards lanes_ltr, some pieces
// seemed to really need to still handle lanes going outward from the "center" line. Should keep
// whittling this down, probably. These very much don't handle multiple direction changes.
impl Road {
    /// These are ordered from closest to center lane (left-most when driving on the right) to
    /// farthest (sidewalk)
    pub(crate) fn children_forwards(&self) -> Vec<(LaneID, LaneType)> {
        let mut result = Vec::new();
        for (l, dir, lt) in self.lanes_ltr() {
            if dir == Direction::Fwd {
                result.push((l, lt));
            }
        }
        result
    }
    pub(crate) fn children_backwards(&self) -> Vec<(LaneID, LaneType)> {
        let mut result = Vec::new();
        for (l, dir, lt) in self.lanes_ltr() {
            if dir == Direction::Back {
                result.push((l, lt));
            }
        }
        result.reverse();
        result
    }

    /// lane must belong to this road. Offset 0 is the centermost lane on each side of a road, then
    /// it counts up from there.
    pub(crate) fn dir_and_offset(&self, lane: LaneID) -> (Direction, usize) {
        for &dir in [Direction::Fwd, Direction::Back].iter() {
            if let Some(idx) = self.children(dir).iter().position(|pair| pair.0 == lane) {
                return (dir, idx);
            }
        }
        panic!("{} doesn't contain {}", self.id, lane);
    }

    // TODO Deprecated
    pub(crate) fn children(&self, dir: Direction) -> Vec<(LaneID, LaneType)> {
        if dir == Direction::Fwd {
            self.children_forwards()
        } else {
            self.children_backwards()
        }
    }

    /// Returns lanes from the "center" going out
    pub(crate) fn incoming_lanes(&self, i: IntersectionID) -> Vec<(LaneID, LaneType)> {
        if self.src_i == i {
            self.children_backwards()
        } else if self.dst_i == i {
            self.children_forwards()
        } else {
            panic!("{} doesn't have an endpoint at {}", self.id, i);
        }
    }

    /// Returns lanes from the "center" going out
    pub(crate) fn outgoing_lanes(&self, i: IntersectionID) -> Vec<(LaneID, LaneType)> {
        if self.src_i == i {
            self.children_forwards()
        } else if self.dst_i == i {
            self.children_backwards()
        } else {
            panic!("{} doesn't have an endpoint at {}", self.id, i);
        }
    }
}
