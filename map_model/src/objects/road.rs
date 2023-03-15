use std::collections::BTreeSet;
use std::fmt;

use anyhow::Result;
use enumset::EnumSet;
use serde::{Deserialize, Serialize};

use abstutil::{deserialize_usize, serialize_usize, Tags};
use geom::{Distance, PolyLine, Polygon, Speed};

use crate::{
    osm, AccessRestrictions, CommonEndpoint, CrossingType, Direction, DrivingSide, IntersectionID,
    Lane, LaneID, LaneSpec, LaneType, Map, PathConstraints, RestrictionType, TransitStopID, Zone,
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

impl RoadID {
    pub fn both_directions(self) -> Vec<DirectedRoadID> {
        vec![
            DirectedRoadID {
                road: self,
                dir: Direction::Fwd,
            },
            DirectedRoadID {
                road: self,
                dir: Direction::Back,
            },
        ]
    }

    pub fn both_sides(self) -> [RoadSideID; 2] {
        [
            RoadSideID {
                road: self,
                side: SideOfRoad::Right,
            },
            RoadSideID {
                road: self,
                side: SideOfRoad::Left,
            },
        ]
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct DirectedRoadID {
    pub road: RoadID,
    pub dir: Direction,
}

impl fmt::Display for DirectedRoadID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "DirectedRoadID({}, {})", self.road.0, self.dir,)
    }
}

impl DirectedRoadID {
    pub fn src_i(self, map: &Map) -> IntersectionID {
        let r = map.get_r(self.road);
        if self.dir == Direction::Fwd {
            r.src_i
        } else {
            r.dst_i
        }
    }

    pub fn dst_i(self, map: &Map) -> IntersectionID {
        let r = map.get_r(self.road);
        if self.dir == Direction::Fwd {
            r.dst_i
        } else {
            r.src_i
        }
    }

    /// Strict for bikes. If there are bike lanes, not allowed to use other lanes.
    pub fn lanes(self, constraints: PathConstraints, map: &Map) -> Vec<LaneID> {
        let r = map.get_r(self.road);
        constraints.filter_lanes(r.children(self.dir).iter().map(|(l, _)| *l).collect(), map)
    }

    /// Get the only sidewalk or shoulder on this side of the road, and panic otherwise.
    pub fn must_get_sidewalk(self, map: &Map) -> LaneID {
        let mut found = Vec::new();
        for (l, lt) in map.get_r(self.road).children(self.dir) {
            if lt.is_walkable() {
                found.push(l);
            }
        }
        if found.len() != 1 {
            panic!(
                "must_get_sidewalk broken by {} ({}). Found lanes {:?}",
                self,
                map.get_r(self.road).orig_id,
                found
            );
        }
        found[0]
    }

    /// Does this directed road have any lanes of a certain type?
    pub fn has_lanes(self, lane_type: LaneType, map: &Map) -> bool {
        for (_, lt) in map.get_r(self.road).children(self.dir) {
            if lt == lane_type {
                return true;
            }
        }
        false
    }
}

/// See https://wiki.openstreetmap.org/wiki/Forward_%26_backward,_left_%26_right.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum SideOfRoad {
    Right,
    Left,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RoadSideID {
    pub road: RoadID,
    pub side: SideOfRoad,
}

impl RoadSideID {
    pub fn get_outermost_lane(self, map: &Map) -> &Lane {
        let r = map.get_r(self.road);
        match self.side {
            SideOfRoad::Right => r.lanes.last().unwrap(),
            SideOfRoad::Left => &r.lanes[0],
        }
    }

    pub fn other_side(self) -> RoadSideID {
        RoadSideID {
            road: self.road,
            side: if self.side == SideOfRoad::Left {
                SideOfRoad::Right
            } else {
                SideOfRoad::Left
            },
        }
    }
}

/// A Road represents a segment between exactly two Intersections. It contains Lanes as children.
#[derive(Serialize, Deserialize, Clone, Debug)]
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
    /// [-1.0, 1.0] theoretically, but in practice, about [-0.25, 0.25]. 0 is flat,
    /// positive is uphill from src_i -> dst_i, negative is downhill.
    pub percent_incline: f64,

    /// Invariant: A road must contain at least one child. These are ordered from the left side of
    /// the road to the right, with that orientation determined by the direction of `center_pts`.
    pub lanes: Vec<Lane>,

    /// The physical center of the road, including sidewalks, after trimming to account for the
    /// intersection geometry. The order implies road orientation.
    pub center_pts: PolyLine,
    /// Like center_pts, but before any trimming for intersection geometry. This is preserved so
    /// that when modifying road width, intersection polygons can be calculated correctly.
    pub untrimmed_center_pts: PolyLine,
    pub trim_start: Distance,
    pub trim_end: Distance,

    pub src_i: IntersectionID,
    pub dst_i: IntersectionID,
    /// Is there a tagged crosswalk near each end of the road?
    pub crosswalk_forward: bool,
    pub crosswalk_backward: bool,

    /// Meaningless order
    pub transit_stops: BTreeSet<TransitStopID>,

    /// Some kind of modal filter or barrier this distance along center_pts.
    pub barrier_nodes: Vec<Distance>,
    /// Some kind of crossing this distance along center_pts.
    pub crossing_nodes: Vec<(Distance, CrossingType)>,
}

impl Road {
    pub(crate) fn lane_specs(&self) -> Vec<LaneSpec> {
        self.lanes
            .iter()
            .map(|l| LaneSpec {
                lt: l.lane_type,
                dir: l.dir,
                width: l.width,
                // TODO These get lost from osm2streets
                allowed_turns: Default::default(),
            })
            .collect()
    }

    pub fn shift_from_left_side(&self, width_from_left_side: Distance) -> Result<PolyLine> {
        self.center_pts
            .shift_from_center(self.get_width(), width_from_left_side)
    }

    /// lane must belong to this road. Offset 0 is the centermost lane on each side of a road, then
    /// it counts up from there. Note this is a different offset than `offset`!
    pub(crate) fn dir_and_offset(&self, lane: LaneID) -> (Direction, usize) {
        for dir in [Direction::Fwd, Direction::Back] {
            if let Some(idx) = self.children(dir).iter().position(|pair| pair.0 == lane) {
                return (dir, idx);
            }
        }
        panic!("{} doesn't contain {}", self.id, lane);
    }

    pub fn parking_to_driving(&self, parking: LaneID) -> Option<LaneID> {
        self.find_closest_lane(parking, |l| l.is_driving())
    }

    pub(crate) fn speed_limit_from_osm(&self) -> Speed {
        if let Some(limit) = self.osm_tags.get("maxspeed") {
            if let Some(speed) = if let Ok(kmph) = limit.parse::<f64>() {
                Some(Speed::km_per_hour(kmph))
            } else if let Some(mph) = limit
                .strip_suffix(" mph")
                .and_then(|x| x.parse::<f64>().ok())
            {
                Some(Speed::miles_per_hour(mph))
            } else {
                None
            } {
                if speed == Speed::ZERO {
                    warn!("{} has a speed limit of 0", self.orig_id.osm_way_id);
                    return Speed::miles_per_hour(1.0);
                }
                return speed;
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
    ) -> Option<LaneID> {
        let our_idx = from.offset as isize;
        self.lanes
            .iter()
            .enumerate()
            .filter_map(|(idx, l)| {
                if (idx as isize) != our_idx && filter(l) {
                    Some((idx, l.id))
                } else {
                    None
                }
            })
            .min_by_key(|(idx, _)| (our_idx - (*idx as isize)).abs())
            .map(|(_, l)| l)
    }

    /// This is the FIRST yellow line where the direction of the road changes. If multiple direction
    /// changes happen, the result is kind of arbitrary.
    pub fn get_dir_change_pl(&self, map: &Map) -> PolyLine {
        let mut found: Option<&Lane> = None;
        for pair in self.lanes.windows(2) {
            if pair[0].dir != pair[1].dir {
                found = Some(&pair[0]);
                break;
            }
        }
        let lane = found.unwrap_or(&self.lanes[0]);
        // There's a weird edge case with single lane light rail on left-handed maps...
        let shifted = if map.get_config().driving_side == DrivingSide::Right || found.is_none() {
            lane.lane_center_pts.must_shift_left(lane.width / 2.0)
        } else {
            lane.lane_center_pts.must_shift_right(lane.width / 2.0)
        };
        if lane.dir == Direction::Fwd {
            shifted
        } else {
            shifted.reversed()
        }
    }

    pub fn get_half_width(&self) -> Distance {
        self.get_width() / 2.0
    }

    pub fn get_width(&self) -> Distance {
        self.lanes.iter().map(|l| l.width).sum::<Distance>()
    }

    pub fn get_thick_polygon(&self) -> Polygon {
        self.center_pts.make_polygons(self.get_width())
    }

    pub fn length(&self) -> Distance {
        self.center_pts.length()
    }

    /// Creates the thick polygon representing one half of the road. For roads with multiple
    /// direction changes (like a two-way cycletrack adjacent to a regular two-way road), the
    /// results are probably weird.
    pub fn get_half_polygon(&self, dir: Direction, map: &Map) -> Result<Polygon> {
        let mut width_fwd = Distance::ZERO;
        let mut width_back = Distance::ZERO;
        for l in &self.lanes {
            if l.dir == Direction::Fwd {
                width_fwd += l.width;
            } else {
                width_back += l.width;
            }
        }
        let center = self.get_dir_change_pl(map);

        // TODO Test on UK maps...
        let shift = if map.get_config().driving_side == DrivingSide::Right {
            1.0
        } else {
            -1.0
        };
        if dir == Direction::Fwd {
            Ok(center
                .shift_right(shift * width_fwd / 2.0)?
                .make_polygons(width_fwd))
        } else {
            Ok(center
                .shift_left(shift * width_back / 2.0)?
                .make_polygons(width_back))
        }
    }

    pub fn get_name(&self, lang: Option<&String>) -> String {
        if let Some(lang) = lang {
            if let Some(name) = self.osm_tags.get(&format!("name:{}", lang)) {
                return name.to_string();
            }
        }

        if let Some(name) = self.osm_tags.get("name") {
            if name.is_empty() {
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

    pub fn get_detailed_rank(&self) -> usize {
        self.osm_tags
            .get(osm::HIGHWAY)
            .map(|hwy| osm::RoadRank::detailed_from_highway(hwy))
            .unwrap_or(0)
    }

    pub fn is_light_rail(&self) -> bool {
        self.lanes.len() == 1 && self.lanes[0].lane_type == LaneType::LightRail
    }

    pub fn is_footway(&self) -> bool {
        self.lanes.len() == 1
            && matches!(
                self.lanes[0].lane_type,
                LaneType::Footway | LaneType::SharedUse
            )
    }

    pub fn is_service(&self) -> bool {
        self.osm_tags.is(osm::HIGHWAY, "service")
    }

    pub fn is_cycleway(&self) -> bool {
        let mut bike = false;
        for lane in &self.lanes {
            if lane.lane_type == LaneType::Biking {
                bike = true;
            } else if !lane.is_walkable() {
                return false;
            }
        }
        bike
    }

    pub fn is_driveable(&self) -> bool {
        self.lanes.iter().any(|l| l.is_driving())
    }

    pub fn common_endpoint(&self, other: &Road) -> CommonEndpoint {
        CommonEndpoint::new((self.src_i, self.dst_i), (other.src_i, other.dst_i))
    }

    pub fn endpoints(&self) -> Vec<IntersectionID> {
        vec![self.src_i, self.dst_i]
    }

    /// Returns the other intersection of this road, panicking if this road doesn't connect to the
    /// input
    /// TODO This should use CommonEndpoint
    pub fn other_endpt(&self, i: IntersectionID) -> IntersectionID {
        if self.src_i == i {
            self.dst_i
        } else if self.dst_i == i {
            self.src_i
        } else {
            panic!("{} doesn't touch {}", self.id, i);
        }
    }

    pub fn is_private(&self) -> bool {
        self.access_restrictions != AccessRestrictions::new() && !self.is_light_rail()
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

    /// Many roads wind up with almost no length, due to their representation in OpenStreetMap. In
    /// reality, these segments are likely located within the interior of an intersection. This
    /// method uses a hardcoded threshold to detect these cases.
    pub fn is_extremely_short(&self) -> bool {
        self.length() < Distance::meters(2.0)
    }

    /// Get the DirectedRoadID pointing to the intersection. Panics if the intersection isn't an
    /// endpoint.
    pub fn directed_id_from(&self, i: IntersectionID) -> DirectedRoadID {
        DirectedRoadID {
            road: self.id,
            dir: if self.src_i == i {
                Direction::Fwd
            } else if self.dst_i == i {
                Direction::Back
            } else {
                panic!("{} doesn't point to {}", self.id, i);
            },
        }
    }

    /// Get the DirectedRoadID pointing from the intersection. Panics if the intersection isn't an
    /// endpoint.
    pub fn directed_id_to(&self, i: IntersectionID) -> DirectedRoadID {
        let mut id = self.directed_id_from(i);
        id.dir = id.dir.opposite();
        id
    }

    pub(crate) fn recreate_lanes(&mut self, lane_specs_ltr: Vec<LaneSpec>) {
        self.lanes.clear();

        let total_width = lane_specs_ltr.iter().map(|x| x.width).sum();

        let mut width_so_far = Distance::ZERO;
        for lane in lane_specs_ltr {
            let id = LaneID {
                road: self.id,
                offset: self.lanes.len(),
            };

            let (src_i, dst_i) = if lane.dir == Direction::Fwd {
                (self.src_i, self.dst_i)
            } else {
                (self.dst_i, self.src_i)
            };

            width_so_far += lane.width / 2.0;
            let pl = self
                .center_pts
                .shift_from_center(total_width, width_so_far)
                .unwrap_or_else(|_| self.center_pts.clone());
            width_so_far += lane.width / 2.0;

            let lane_center_pts = if lane.dir == Direction::Fwd {
                pl
            } else {
                pl.reversed()
            };

            self.lanes.push(Lane {
                id,
                lane_center_pts,
                width: lane.width,
                src_i,
                dst_i,
                lane_type: lane.lt,
                dir: lane.dir,
                driving_blackhole: false,
                biking_blackhole: false,
            });
        }
    }

    /// Returns all lanes located between l1 and l2, exclusive.
    pub fn get_lanes_between(&self, l1: LaneID, l2: LaneID) -> Vec<LaneID> {
        let mut results = Vec::new();
        let mut found_start = false;
        for l in &self.lanes {
            if found_start {
                if l.id == l1 || l.id == l2 {
                    return results;
                }
                results.push(l.id);
            } else if l.id == l1 || l.id == l2 {
                found_start = true;
            }
        }
        panic!("{} doesn't contain both {} and {}", self.id, l1, l2);
    }

    /// A simple classification of if the directed road is stressful or not for cycling. Arterial
    /// roads without a bike lane match this. Why arterial, instead of looking at speed limits?
    /// Even on arterial roads with official speed limits lowered, in practice vehicles still
    /// travel at the speed suggested by the design of the road.
    // TODO Should elevation matter or not? Flat high-speed roads are still terrifying, but there's
    // something about slogging up (or flying down!) a pothole-filled road inches from cars.
    pub fn high_stress_for_bikes(&self, map: &Map, dir: Direction) -> bool {
        let mut bike_lanes = false;
        let mut can_use = false;
        // Can a bike even use it, or is it a highway?
        for l in &self.lanes {
            if l.lane_type == LaneType::Biking && l.dir == dir {
                bike_lanes = true;
            }
            if PathConstraints::Bike.can_use(l, map) {
                can_use = true;
            }
        }
        if !can_use || bike_lanes {
            return false;
        }
        self.get_rank() != osm::RoadRank::Local
    }

    pub fn oneway_for_driving(&self) -> Option<Direction> {
        LaneSpec::oneway_for_driving(&self.lane_specs())
    }

    /// Does either end of this road lead nowhere for cars?
    /// (Asking this for a non-driveable road may be kind of meaningless)
    pub fn is_deadend_for_driving(&self, map: &Map) -> bool {
        map.get_i(self.src_i).is_deadend_for_driving(map)
            || map.get_i(self.dst_i).is_deadend_for_driving(map)
    }
}

// TODO All of this is kind of deprecated? Some callers seem to really need to still handle lanes
// going outward from the "center" line. Should keep whittling this down, probably. These very much
// don't handle multiple direction changes.
impl Road {
    /// These are ordered from closest to center lane (left-most when driving on the right) to
    /// farthest (sidewalk)
    pub(crate) fn children_forwards(&self) -> Vec<(LaneID, LaneType)> {
        let mut result = Vec::new();
        for l in &self.lanes {
            if l.dir == Direction::Fwd {
                result.push((l.id, l.lane_type));
            }
        }
        result
    }
    pub(crate) fn children_backwards(&self) -> Vec<(LaneID, LaneType)> {
        let mut result = Vec::new();
        for l in &self.lanes {
            if l.dir == Direction::Back {
                result.push((l.id, l.lane_type));
            }
        }
        result.reverse();
        result
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
}

/// Refers to a road segment between two nodes, using OSM IDs. Note OSM IDs are not stable over
/// time and the relationship between a road/intersection and way/node isn't 1:1 at all.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OriginalRoad {
    pub osm_way_id: osm::WayID,
    pub i1: osm::NodeID,
    pub i2: osm::NodeID,
}

impl fmt::Display for OriginalRoad {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "OriginalRoad({} from {} to {}",
            self.osm_way_id, self.i1, self.i2
        )
    }
}
impl fmt::Debug for OriginalRoad {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self)
    }
}

impl OriginalRoad {
    pub fn new(way: i64, (i1, i2): (i64, i64)) -> OriginalRoad {
        OriginalRoad {
            osm_way_id: osm::WayID(way),
            i1: osm::NodeID(i1),
            i2: osm::NodeID(i2),
        }
    }
}
