//! This crate provides a Rust interface to different parts of the [SUMO](https://www.eclipse.org/sumo/) traffic simulator.

use std::collections::BTreeMap;

use geom::{Distance, PolyLine, Polygon, Pt2D, Speed};

pub use self::raw::{EdgeID, LaneID, NodeID};

mod normalize;
mod raw;

/// A normalized form of a SUMO
/// [network](https://sumo.dlr.de/docs/Networks/SUMO_Road_Networks.html). A `raw::Network` is a direct representation of a .net.xml file. That's further simplified to produce this structure, which should be easier to work with. The
/// transformations:
///
/// - Any unspecified edge and lane attributes are inherited from `types` or set to defaults
/// - Edges without a `from` or `to` are filtered out
/// - Internal edges and junctions are filtered out
/// - The Y coordinate is inverted, so that Y decreases northbound
pub struct Network {
    pub location: raw::Location,
    pub edges: BTreeMap<EdgeID, Edge>,
    pub junctions: BTreeMap<NodeID, Junction>,
}

pub struct Edge {
    pub id: EdgeID,
    pub edge_type: String,
    pub name: Option<String>,
    pub from: NodeID,
    pub to: NodeID,
    pub priority: usize,
    pub lanes: Vec<Lane>,
    pub center_line: PolyLine,
}

pub struct Lane {
    pub id: LaneID,
    /// 0 is the rightmost lane
    pub index: usize,
    pub speed: Speed,
    pub length: Distance,
    pub width: Distance,
    pub center_line: PolyLine,
    pub allow: Vec<String>,
}

pub struct Junction {
    pub id: NodeID,
    pub junction_type: String,
    pub pt: Pt2D,
    pub incoming_lanes: Vec<LaneID>,
    pub internal_lanes: Vec<LaneID>,
    pub shape: Polygon,
}
