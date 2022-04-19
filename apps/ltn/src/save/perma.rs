use serde::{Deserialize, Serialize};

use abstio::MapName;

// TODO Can we use macros and walk the struct, transforming some of the fields?
// Or the lazier approach -- save the proposal as it is, but include the mapping from IDs to
// permanent OSM IDs? But then on the other side, we still have to walk the proposal and modify
// everything.
//
// So look into macros...
//
// Don't we have to generate...
//
// - a second copy of each struct, with its own serde
// - a way to copy map to perma, doing translation along the way
// - a way to copy perma to map, doing translation
// ... also reaching into Vec, BTreeMap, etc along the way?
//
// Look for reflection-style walking?

#[derive(Serialize, Deserialize, Clone)]
struct PermanentProposal {
    map: MapName,
    name: String,
    abst_version: String,

    partitioning: Partitioning,
    modal_filters: ModalFilters,
}

struct PermanentFilters {
    roads: BTreeMap<RoadID, Distance>,
    intersections: BTreeMap<IntersectionID, PermanentDiagonalFilter>,
}

struct DiagonalFilter {
    r1: RoadID,
    r2: RoadID,
    i: IntersectionID,

    group1: BTreeSet<RoadID>,
    group2: BTreeSet<RoadID>,
}
