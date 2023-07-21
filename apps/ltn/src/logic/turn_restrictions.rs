use map_model::{Map, RoadID, IntersectionID, Intersection, TurnType};
use osm2streets::{Direction, RestrictionType};
use widgetry::GeomBatch;

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
pub fn destination_roads(map: &Map, source_r_id: &RoadID) -> Vec<RoadID> {
    let source_r = map.get_r(*source_r_id);
    let mut destinations: Vec<RoadID> = Vec::new();
    
    let one_way = source_r.oneway_for_driving();

    if one_way != Some(Direction::Fwd) {
        for r in & map.get_i(source_r.src_i).roads {
            if source_r.id != *r {
                destinations.push(*r);
            }
        }
    }

    if one_way != Some(Direction::Back) {
        for r in & map.get_i(source_r.dst_i).roads {
            if source_r.id != *r {
                destinations.push(*r);
            }
        }
    }
    destinations
}
