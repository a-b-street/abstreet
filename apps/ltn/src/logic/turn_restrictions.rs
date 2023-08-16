use std::collections::HashSet;

use map_model::{Map, RoadID, IntersectionID};
use osm2streets::{Direction, RestrictionType};


// TODO This should probably move to osm2streets
// TODO TurnRestrictions is still incomplete so causes compilation problems
// pub struct TurnRestrictions {
//     pub source_r: &RoadID,
//     pub restriction_type: RestrictionType,
//     pub turn_type: TurnType,
//     pub icon: GeomBatch,
//     pub dest_r: &RoadID,
// }

// impl TurnRestrictions {
//     pub fn new(source_r: &RoadID, dest_r: &RoadID) {
//         let tr = Self {
//             source_r: source_r,
//             // For now assign simple values here, which we'll override next
//             restriction_type: RestrictionType::BanTurns,
//             turn_type: TurnType::UnmarkedCrossing,
//             icon: GeomBatch::new(),
//             dest_r: dest_r,
//         };
//
//         // Now calculate the correct values for the various params
//         tr.XXX
//     }
// }

/// Returns a Vec<RoadID> of all roads that are possible destinations from the given source road, accounting for one-way restrictions.
///
// TODO exclude incoming one-way roads - add test cases
// 1. T-junction: Three-way intersection, all roads are two way.
// 2. Four-way, with two one-way roads, both approaching the intersection
// 3. Four-way, with two one-way roads, one approaching and one leaving the intersection
// 4. Something with a U-Turn
// TODO highlighting possible destinations for complex turns
// TODO highlight possible roads leading away form the neighbourhood
// TODO clickable/mouseover area to equal sign icon, not just the road geom.

pub fn restricted_destination_roads(map: &Map, source_r_id: RoadID, i: Option<IntersectionID>) -> HashSet<RoadID> {
    let candidate_roads = destination_roads(map, source_r_id, i);

    let road = map.get_r(source_r_id);
    let mut restricted_destinations: HashSet<RoadID> = HashSet::new();
        
    for (restriction, r2) in &road.turn_restrictions {
        if *restriction == RestrictionType::BanTurns && candidate_roads.contains(r2) {
            restricted_destinations.insert(*r2);
        }
    }
    for (via, r2) in &road.complicated_turn_restrictions {
        if candidate_roads.contains(via) {
            restricted_destinations.insert(*via);
            restricted_destinations.insert(*r2);
        }
    }
    restricted_destinations
}

/// checks that an Intersection ID is connected to a RoadID. Returns `true` if connected, `false` otherwise.
fn verify_intersection(map: &Map, r: RoadID, i: IntersectionID) -> bool {
    let road = map.get_r(r);
    return road.dst_i == i || road.src_i == i
}

/// `i` is Options. If `i` is `Some` then, it must be connected to `source_r_id`. It is used to filter
/// the results to return only the destination roads that connect to `i`.
pub fn destination_roads(map: &Map, source_r_id: RoadID, i: Option<IntersectionID>) -> HashSet<RoadID> {

    if i.is_some() && !verify_intersection(map, source_r_id, i.unwrap()){
        panic!("IntersectionID {:?}, does not connect to RoadID {:?}", i, source_r_id);
    }

    let source_r = map.get_r(source_r_id);
    let mut destinations: HashSet<RoadID> = HashSet::new();

    let one_way = source_r.oneway_for_driving();

    if one_way != Some(Direction::Fwd) && Some(source_r.dst_i) != i {
        for r in &map.get_i(source_r.src_i).roads {
            if source_r.id != *r {
                destinations.insert(*r);
            }
        }
    }

    if one_way != Some(Direction::Back) && Some(source_r.src_i) != i {
        for r in &map.get_i(source_r.dst_i).roads {
            if source_r.id != *r {
                destinations.insert(*r);
            }
        }
    }
    destinations
}
