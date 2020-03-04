use serde_derive::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Serialize, Deserialize)]
pub struct TrafficSignal {
    /// The ID of the OSM node representing the intersection with the traffic signal. This node
    /// should be tagged `highway = traffic_signals` in OSM.
    pub intersection_osm_node_id: i64,
    /// The traffic signal repeatedly cycles through these phases. During each phase, only some
    /// turns are protected and permitted through the intersection.
    pub phases: Vec<Phase>,
    // TODO What should this be relative to?
    pub offset_seconds: usize,
}

#[derive(Serialize, Deserialize)]
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
#[derive(Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
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
#[derive(Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
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

static DATA: include_dir::Dir = include_dir::include_dir!("data");

/// Returns all traffic signal data compiled into this build, keyed by OSM node ID.
pub fn load_all_data() -> Result<BTreeMap<i64, TrafficSignal>, std::io::Error> {
    let mut results = BTreeMap::new();
    if let Some(dir) = DATA.get_dir("data") {
        for f in dir.files() {
            let ts: TrafficSignal = serde_json::from_slice(&f.contents())?;
            results.insert(ts.intersection_osm_node_id, ts);
        }
    }
    Ok(results)
}
