use std::collections::BTreeSet;
use std::fmt;

use serde::{Deserialize, Serialize};

use abstutil::{deserialize_usize, serialize_usize, wraparound_get, Tags};
use geom::{Distance, Line, PolyLine, Polygon, Pt2D, Ring};

use crate::{
    osm, BusStopID, DirectedRoadID, Direction, IntersectionID, Map, MapConfig, Road, RoadID,
    TurnType,
};

/// From some manually audited cases in Seattle, the length of parallel street parking spots is a
/// bit different than the length in parking lots, so set a different value here.
pub const PARKING_LOT_SPOT_LENGTH: Distance = Distance::const_meters(6.4);

pub const NORMAL_LANE_THICKNESS: Distance = Distance::const_meters(2.5);
const SERVICE_ROAD_LANE_THICKNESS: Distance = Distance::const_meters(1.5);
pub const SIDEWALK_THICKNESS: Distance = Distance::const_meters(1.5);
const SHOULDER_THICKNESS: Distance = Distance::const_meters(0.5);

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
    Buffer(BufferType),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum BufferType {
    /// Just paint!
    Stripes,
    /// Flex posts, wands, cones, other "weak" forms of protection. Can weave through them.
    FlexPosts,
    /// Sturdier planters, with gaps.
    Planters,
    /// Solid barrier, no gaps.
    JerseyBarrier,
    /// A raised curb
    Curb,
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
            LaneType::Buffer(_) => false,
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
            LaneType::Buffer(_) => false,
        }
    }

    pub fn is_walkable(self) -> bool {
        self == LaneType::Sidewalk || self == LaneType::Shoulder
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
            LaneType::Buffer(BufferType::Stripes) => "striped pavement",
            LaneType::Buffer(BufferType::FlexPosts) => "flex post barriers",
            LaneType::Buffer(BufferType::Planters) => "planter barriers",
            LaneType::Buffer(BufferType::JerseyBarrier) => "a Jersey barrier",
            LaneType::Buffer(BufferType::Curb) => "a raised curb",
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
            LaneType::Buffer(BufferType::Stripes) => "stripes",
            LaneType::Buffer(BufferType::FlexPosts) => "flex posts",
            LaneType::Buffer(BufferType::Planters) => "planters",
            LaneType::Buffer(BufferType::JerseyBarrier) => "Jersey barrier",
            LaneType::Buffer(BufferType::Curb) => "curb",
        }
    }

    pub fn from_short_name(x: &str) -> Option<LaneType> {
        match x {
            "driving lane" => Some(LaneType::Driving),
            "bike lane" => Some(LaneType::Biking),
            "bus lane" => Some(LaneType::Bus),
            "parking lane" => Some(LaneType::Parking),
            "sidewalk" => Some(LaneType::Sidewalk),
            "shoulder" => Some(LaneType::Shoulder),
            "left-turn lane" => Some(LaneType::SharedLeftTurn),
            "construction" => Some(LaneType::Construction),
            "light rail track" => Some(LaneType::LightRail),
            "stripes" => Some(LaneType::Buffer(BufferType::Stripes)),
            "flex posts" => Some(LaneType::Buffer(BufferType::FlexPosts)),
            "planters" => Some(LaneType::Buffer(BufferType::Planters)),
            "Jersey barrier" => Some(LaneType::Buffer(BufferType::JerseyBarrier)),
            "curb" => Some(LaneType::Buffer(BufferType::Curb)),
            _ => None,
        }
    }

    /// Represents the lane type as a single character, for use in tests.
    pub fn to_char(self) -> char {
        match self {
            LaneType::Driving => 'd',
            LaneType::Biking => 'b',
            LaneType::Bus => 'B',
            LaneType::Parking => 'p',
            LaneType::Sidewalk => 's',
            LaneType::Shoulder => 'S',
            LaneType::SharedLeftTurn => 'C',
            LaneType::Construction => 'x',
            LaneType::LightRail => 'l',
            LaneType::Buffer(_) => '|',
        }
    }

    /// The inverse of `to_char`. Always picks one buffer type. Panics on invalid input.
    pub fn from_char(x: char) -> LaneType {
        match x {
            'd' => LaneType::Driving,
            'b' => LaneType::Biking,
            'B' => LaneType::Bus,
            'p' => LaneType::Parking,
            's' => LaneType::Sidewalk,
            'S' => LaneType::Shoulder,
            'C' => LaneType::SharedLeftTurn,
            'x' => LaneType::Construction,
            'l' => LaneType::LightRail,
            '|' => LaneType::Buffer(BufferType::FlexPosts),
            _ => panic!("from_char({}) undefined", x),
        }
    }
}

/// A road segment is broken down into individual lanes, which have a LaneType.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Lane {
    pub id: LaneID,
    pub parent: RoadID,
    pub lane_type: LaneType,
    pub lane_center_pts: PolyLine,
    pub width: Distance,
    pub dir: Direction,

    pub src_i: IntersectionID,
    pub dst_i: IntersectionID,

    /// Meaningless order
    pub bus_stops: BTreeSet<BusStopID>,

    /// {Cars, bikes} trying to start or end here might not be able to reach most lanes in the
    /// graph, because this is near a border.
    pub driving_blackhole: bool,
    pub biking_blackhole: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LaneSpec {
    pub lt: LaneType,
    pub dir: Direction,
    pub width: Distance,
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
            id: self.parent,
            dir: self.dir,
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

    /// Starting from this lane, follow the lane's left edge to the intersection, continuing to
    /// "walk around the block" until we reach the starting point. This only makes sense for the
    /// outermost lanes on a road. Returns the polygon and all visited lanes.
    ///
    /// TODO This process currently fails for some starting positions; orienting is weird.
    pub fn trace_around_block(&self, map: &Map) -> Option<(Polygon, BTreeSet<LaneID>)> {
        let start = self.id;
        let mut pts = Vec::new();
        let mut current = start;
        let mut fwd = map.get_parent(start).lanes_ltr()[0].0 == start;
        let mut visited = BTreeSet::new();
        loop {
            let l = map.get_l(current);
            let lane_pts = if fwd {
                l.lane_center_pts.shift_left(l.width / 2.0)
            } else {
                l.lane_center_pts.reversed().shift_left(l.width / 2.0)
            }
            .unwrap()
            .into_points();
            if let Some(last_pt) = pts.last().cloned() {
                if last_pt != lane_pts[0] {
                    let last_i = if fwd { l.src_i } else { l.dst_i };
                    if let Some(pl) = map
                        .get_i(last_i)
                        .polygon
                        .clone()
                        .into_ring()
                        .get_shorter_slice_btwn(last_pt, lane_pts[0])
                    {
                        pts.extend(pl.into_points());
                    }
                }
            }
            pts.extend(lane_pts);
            // Imagine pointing down this lane to the intersection. Rotate left -- which road is
            // next?
            let i = if fwd { l.dst_i } else { l.src_i };
            // TODO Remove these debug statements entirely after stabilizing this
            //println!("{}, fwd={}, pointing to {}", current, fwd, i);
            let mut roads = map
                .get_i(i)
                .get_roads_sorted_by_incoming_angle(map.all_roads());
            roads.retain(|r| !map.get_r(*r).is_footway());
            let idx = roads.iter().position(|r| *r == l.parent).unwrap();
            // Get the next road counter-clockwise
            let next_road = map.get_r(*wraparound_get(&roads, (idx as isize) + 1));
            // Depending on if this road points to or from the intersection, get the left- or
            // right-most lane.
            let next_lane = if next_road.src_i == i {
                next_road.lanes_ltr()[0].0
            } else {
                next_road.lanes_ltr().last().unwrap().0
            };
            if next_lane == start {
                break;
            }
            if visited.contains(&current) {
                //println!("Loop, something's broken");
                return None;
            }
            visited.insert(current);
            current = next_lane;
            fwd = map.get_l(current).src_i == i;
        }
        pts.push(pts[0]);
        pts.dedup();
        Some((Ring::new(pts).ok()?.into_polygon(), visited))
    }
}

impl LaneSpec {
    /// For a given lane type, returns some likely widths. This may depend on the type of the road,
    /// so the OSM tags are also passed in. The first value returned will be used as a default.
    pub fn typical_lane_widths(lt: LaneType, tags: &Tags) -> Vec<(Distance, &'static str)> {
        // These're cobbled together from various sources
        match lt {
            // https://en.wikipedia.org/wiki/Lane#Lane_width
            LaneType::Driving => {
                let mut choices = vec![
                    (Distance::feet(8.0), "narrow"),
                    (SERVICE_ROAD_LANE_THICKNESS, "alley"),
                    (Distance::feet(10.0), "typical"),
                    (Distance::feet(12.0), "highway"),
                ];
                if tags.is(osm::HIGHWAY, "service") || tags.is("narrow", "yes") {
                    choices.swap(1, 0);
                }
                choices
            }
            // https://www.gov.uk/government/publications/cycle-infrastructure-design-ltn-120 table
            // 5-2
            LaneType::Biking => vec![
                (Distance::meters(2.0), "standard"),
                (Distance::meters(1.5), "absolute minimum"),
            ],
            // https://nacto.org/publication/urban-street-design-guide/street-design-elements/transit-streets/dedicated-curbside-offset-bus-lanes/
            LaneType::Bus => vec![
                (Distance::feet(12.0), "normal"),
                (Distance::feet(10.0), "minimum"),
            ],
            // https://nacto.org/publication/urban-street-design-guide/street-design-elements/lane-width/
            LaneType::Parking => {
                let mut choices = vec![
                    (Distance::feet(7.0), "narrow"),
                    (SERVICE_ROAD_LANE_THICKNESS, "alley"),
                    (Distance::feet(9.0), "wide"),
                    (Distance::feet(15.0), "loading zone"),
                ];
                if tags.is(osm::HIGHWAY, "service") || tags.is("narrow", "yes") {
                    choices.swap(1, 0);
                }
                choices
            }
            // Just a guess
            LaneType::SharedLeftTurn => vec![(NORMAL_LANE_THICKNESS, "default")],
            // These're often converted from existing lanes, so just retain that width
            LaneType::Construction => vec![(NORMAL_LANE_THICKNESS, "default")],
            // No idea, just using this for now...
            LaneType::LightRail => vec![(NORMAL_LANE_THICKNESS, "default")],
            // http://www.seattle.gov/rowmanual/manual/4_11.asp
            LaneType::Sidewalk => vec![
                (SIDEWALK_THICKNESS, "default"),
                (Distance::feet(6.0), "wide"),
            ],
            LaneType::Shoulder => vec![(SHOULDER_THICKNESS, "default")],
            // Pretty wild guesses
            LaneType::Buffer(BufferType::Stripes) => vec![(Distance::meters(1.5), "default")],
            LaneType::Buffer(BufferType::FlexPosts) => {
                vec![(Distance::meters(1.5), "default")]
            }
            LaneType::Buffer(BufferType::Planters) => {
                vec![(Distance::meters(2.0), "default")]
            }
            LaneType::Buffer(BufferType::JerseyBarrier) => {
                vec![(Distance::meters(1.5), "default")]
            }
            LaneType::Buffer(BufferType::Curb) => vec![(Distance::meters(0.5), "default")],
        }
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
