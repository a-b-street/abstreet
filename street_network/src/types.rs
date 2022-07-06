use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use abstutil::{deserialize_btreemap, serialize_btreemap, Tags};
use geom::Distance;

use crate::{osm, OriginalRoad};

pub const NORMAL_LANE_THICKNESS: Distance = Distance::const_meters(2.5);
const SERVICE_ROAD_LANE_THICKNESS: Distance = Distance::const_meters(1.5);
pub const SIDEWALK_THICKNESS: Distance = Distance::const_meters(1.5);
const SHOULDER_THICKNESS: Distance = Distance::const_meters(0.5);

/// None corresponds to the native name
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct NamePerLanguage(
    #[serde(
        serialize_with = "serialize_btreemap",
        deserialize_with = "deserialize_btreemap"
    )]
    pub(crate) BTreeMap<Option<String>, String>,
);

impl NamePerLanguage {
    pub fn get(&self, lang: Option<&String>) -> &String {
        // TODO Can we avoid this clone?
        let lang = lang.cloned();
        if let Some(name) = self.0.get(&lang) {
            return name;
        }
        &self.0[&None]
    }

    pub fn new(tags: &Tags) -> Option<NamePerLanguage> {
        let native_name = tags.get(osm::NAME)?;
        let mut map = BTreeMap::new();
        map.insert(None, native_name.to_string());
        for (k, v) in tags.inner() {
            if let Some(lang) = k.strip_prefix("name:") {
                map.insert(Some(lang.to_string()), v.to_string());
            }
        }
        Some(NamePerLanguage(map))
    }

    pub fn unnamed() -> NamePerLanguage {
        let mut map = BTreeMap::new();
        map.insert(None, "unnamed".to_string());
        NamePerLanguage(map)
    }

    pub fn languages(&self) -> Vec<&String> {
        self.0.keys().flatten().collect()
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MapConfig {
    /// If true, driving happens on the right side of the road (USA). If false, on the left
    /// (Australia).
    pub driving_side: DrivingSide,
    pub bikes_can_use_bus_lanes: bool,
    /// If true, roads without explicitly tagged sidewalks may have sidewalks or shoulders. If
    /// false, no sidewalks will be inferred if not tagged in OSM, and separate sidewalks will be
    /// included.
    pub inferred_sidewalks: bool,
    /// Street parking is divided into spots of this length. 8 meters is a reasonable default, but
    /// people in some regions might be more accustomed to squeezing into smaller spaces. This
    /// value can be smaller than the hardcoded maximum car length; cars may render on top of each
    /// other, but otherwise the simulation doesn't care.
    pub street_parking_spot_length: Distance,
    /// If true, turns on red which do not conflict crossing traffic ('right on red') are allowed
    pub turn_on_red: bool,

    /// Enable experimental dog-leg intersection merging
    pub find_dog_legs_experiment: bool,
    /// Experimentally merge these OSM ways
    pub merge_osm_ways: Vec<OriginalRoad>,
}

impl MapConfig {
    pub fn default_for_side(driving_side: DrivingSide) -> Self {
        Self {
            driving_side,
            bikes_can_use_bus_lanes: true,
            inferred_sidewalks: true,
            street_parking_spot_length: Distance::meters(8.0),
            turn_on_red: true,
            find_dog_legs_experiment: false,
            merge_osm_ways: Vec::new(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq)]
pub enum DrivingSide {
    Right,
    Left,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum IntersectionType {
    StopSign,
    TrafficSignal,
    Border,
    Construction,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LaneSpec {
    pub lt: LaneType,
    pub dir: Direction,
    pub width: Distance,
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

    /// Put a list of forward and backward lanes into left-to-right order, depending on the driving
    /// side. Both input lists should be ordered from the center of the road going outwards.
    pub(crate) fn assemble_ltr(
        mut fwd_side: Vec<LaneSpec>,
        mut back_side: Vec<LaneSpec>,
        driving_side: DrivingSide,
    ) -> Vec<LaneSpec> {
        match driving_side {
            DrivingSide::Right => {
                back_side.reverse();
                back_side.extend(fwd_side);
                back_side
            }
            DrivingSide::Left => {
                fwd_side.reverse();
                fwd_side.extend(back_side);
                fwd_side
            }
        }
    }

    /// None if bidirectional. If it's one-way, which direction is that relative to the road?
    /// (Usually forwards)
    pub fn oneway_for_driving(lanes: &[LaneSpec]) -> Option<Direction> {
        let mut fwd = false;
        let mut back = false;
        for x in lanes {
            if x.lt == LaneType::Driving {
                if x.dir == Direction::Fwd {
                    fwd = true;
                } else {
                    back = true;
                }
            }
        }
        if fwd && back {
            // Bidirectional
            None
        } else if fwd {
            Some(Direction::Fwd)
        } else if back {
            Some(Direction::Back)
        } else {
            // Not driveable at all
            None
        }
    }
}
