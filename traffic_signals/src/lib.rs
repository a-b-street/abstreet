use serde_derive::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TrafficSignal {
    /// The ID of the OSM node representing the intersection with the traffic signal. This node
    /// should be tagged `highway = traffic_signals` in OSM.
    pub intersection_osm_node_id: i64,
    /// The traffic signal repeatedly cycles through these phases. During each phase, only some
    /// turns are protected and permitted through the intersection.
    pub phases: Vec<Phase>,
    // TODO What should this be relative to?
    pub offset_seconds: usize,
    /// Information about the person mapping the signal.
    pub observed: Metadata,
    /// Information about the latest person to verify a previously mapped signal.
    pub audited: Option<Metadata>,
}

/// A traffic signal is in one phase at any time. The phase describes what movements are possible.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Phase {
    /// During this phase, these turns can be performed with the highest priority, protected by a
    /// green light. No two protected turns in the same phase should cross; that would be a
    /// conflict.
    pub protected_turns: BTreeSet<Turn>,
    /// During this phase, these turns can be performed after yielding. For example, an unprotected
    /// left turn after yielding to oncoming traffic, or a right turn on red after yielding to
    /// oncoming traffic and crosswalks.
    pub permitted_turns: BTreeSet<Turn>,
    /// The phase lasts this long before moving to the next one.
    pub duration_seconds: usize,
}

/// A movement through an intersection.
///
/// TODO Diagram of the 4 crosswalk cases.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Turn {
    /// The movement begins at the end of this road segment.
    pub from: DirectedRoad,
    /// The movement ends at the beginning of this road segment.
    pub to: DirectedRoad,
    /// The ID of the OSM node representing the intersection. This is redundant for turns performed
    /// by vehicles, but is necessary for disambiguating the 4 cases of crosswalks.
    pub intersection_osm_node_id: i64,
    /// True iff the movement is along a crosswalk. Note that moving over a crosswalk has a
    /// different `Turn` for each direction.
    pub is_crosswalk: bool,
}

/// A road segment connecting two intersections, and a direction along the segment.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DirectedRoad {
    /// The ID of the OSM way representing the road.
    pub osm_way_id: i64,
    /// The ID of the OSM node at the start of this road segment.
    pub osm_node1: i64,
    /// The ID of the OSM node at the end of this road segment.
    pub osm_node2: i64,
    /// The direction along the road segment. See
    /// https://wiki.openstreetmap.org/wiki/Forward_%26_backward,_left_%26_right for details.
    pub is_forwards: bool,
}

/// Extra information about the occasion that a signal was mapped or audited.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Metadata {
    /// Name, email, or whatever else somebody wants to use to identify themselves. This can be
    /// left blank; there's no obligation to reveal any information about yourself.
    pub author: String,
    /// The time when the signal was mapped or audited in TODO format. This is useful to determine
    /// if some signals operate on a different plan on weekends, late at night, during rush hour,
    /// etc.
    pub datetime: String,
    /// Any other relevant notes or observations.
    pub notes: String,
}

static DATA: include_dir::Dir = include_dir::include_dir!("src/data");

/// Returns all traffic signal data compiled into this build, keyed by OSM node ID.
// TODO Use a build script to do this. But have to generate Rust code to populate the struct? For
// now, the data directory is in src/ so changes to it trigger rebuild.
pub fn load_all_data() -> Result<BTreeMap<i64, TrafficSignal>, std::io::Error> {
    let mut results = BTreeMap::new();
    for f in DATA.files() {
        let ts: TrafficSignal = serde_json::from_slice(&f.contents())?;
        results.insert(ts.intersection_osm_node_id, ts);
    }
    Ok(results)
}
