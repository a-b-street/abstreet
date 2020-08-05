// These are common OSM keys. Keys used in just one or two places don't really need to be defined
// here.

// These're normal OSM keys.
pub const NAME: &str = "name";
pub const HIGHWAY: &str = "highway";
pub const MAXSPEED: &str = "maxspeed";
pub const PARKING_RIGHT: &str = "parking:lane:right";
pub const PARKING_LEFT: &str = "parking:lane:left";
pub const PARKING_BOTH: &str = "parking:lane:both";
pub const SIDEWALK: &str = "sidewalk";

// The rest of these are all inserted by A/B Street to plumb data between different stages of map
// construction. They could be plumbed another way, but this is the most convenient.

// Just a copy of OSM IDs, so that things displaying/searching tags will also pick these up.
pub const OSM_WAY_ID: &str = "abst:osm_way_id";
pub const OSM_REL_ID: &str = "abst:osm_rel_id";
// OSM ways are split into multiple roads. The first and last road are marked, which is important
// for interpreting turn restrictions.
pub const ENDPT_FWD: &str = "abst:endpt_fwd";
pub const ENDPT_BACK: &str = "abst:endpt_back";

// Any roads might have these.
pub const INFERRED_PARKING: &str = "abst:parking_inferred";
pub const INFERRED_SIDEWALKS: &str = "abst:sidewalks_inferred";

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone)]
pub enum RoadRank {
    Local,
    Arterial,
    Highway,
}

impl RoadRank {
    pub fn from_highway(hwy: &str) -> RoadRank {
        match hwy {
            "motorway" | "motorway_link" => RoadRank::Highway,
            "trunk" | "trunk_link" => RoadRank::Highway,
            "primary" | "primary_link" => RoadRank::Arterial,
            "secondary" | "secondary_link" => RoadRank::Arterial,
            "tertiary" | "tertiary_link" => RoadRank::Arterial,
            _ => RoadRank::Local,
        }
    }
}
