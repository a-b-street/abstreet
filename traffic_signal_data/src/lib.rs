//! A representation of traffic signal configuration that references OpenStreetMap IDs and is
//! hopefully robust over minor edits over time.

use serde_derive::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct TrafficSignal {
    /// The ID of the OSM node representing the intersection with the traffic signal. This node
    /// should be tagged `highway = traffic_signals` in OSM.
    pub intersection_osm_node_id: i64,
    /// The traffic signal repeatedly cycles through these stages. During each stage, only some
    /// turns are protected and permitted through the intersection.
    pub stages: Vec<Stage>,
    /// Relative to a central clock, delay the first stage by this many seconds.
    pub offset_seconds: usize,
}

/// A traffic signal is in one stage at any time. The stage describes what movements are possible.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Stage {
    /// During this stage, these turns can be performed with the highest priority, protected by a
    /// green light. No two protected turns in the same stage should cross; that would be a
    /// conflict.
    pub protected_turns: BTreeSet<Turn>,
    /// During this stage, these turns can be performed after yielding. For example, an unprotected
    /// left turn after yielding to oncoming traffic, or a right turn on red after yielding to
    /// oncoming traffic and crosswalks.
    pub permitted_turns: BTreeSet<Turn>,
    /// The stage lasts this long before moving to the next one.
    pub stage_type: StageType,
}

/// How long a stage lasts before moving to the next one.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum StageType {
    /// A fixed number of seconds.
    Fixed(usize),
    /// Minimum, Delay, Additional
    /// Minimum is the minimum cycle duration, 0 allows it to be skipped if no demand.
    /// Delay is the duration with no demand needed to end a cycle, 0 ends as soon as there is no
    /// demand. Additional is the maximum additional duration for an extended cycle. If minimum
    /// is 20, and additional is 40, the maximum cycle duration is 60.
    /// If there are crosswalks, the minimum is the minimum for the maximum crosswalks
    Variable(usize, usize, usize),
}

/// A movement through an intersection.
///
/// Movements over crosswalks are a little confusing to understand. See the crosswalk_turns.png
/// diagram in this repository for some clarification.
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

static DATA: include_dir::Dir = include_dir::include_dir!("data");

/// Returns all traffic signal data compiled into this build, keyed by OSM node ID.
// TODO Use a build script to do this. But have to generate Rust code to populate the struct?
pub fn load_all_data() -> Result<BTreeMap<i64, TrafficSignal>, std::io::Error> {
    let mut results = BTreeMap::new();
    for f in DATA.files() {
        let ts: TrafficSignal = serde_json::from_slice(&f.contents())?;
        results.insert(ts.intersection_osm_node_id, ts);
    }
    Ok(results)
}
