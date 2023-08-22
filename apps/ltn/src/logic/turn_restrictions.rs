use std::collections::HashSet;

use map_model::{Map, RoadID, IntersectionID};
use osm2streets::{Direction, RestrictionType};
use geom::{Polygon, Pt2D};

pub struct FocusedTurns {
    pub src_r: RoadID,
    pub i: IntersectionID,
    pub hull: Polygon,
    pub permitted_t: HashSet<RoadID>,
    pub prohibited_t: HashSet<RoadID>,
}

impl FocusedTurns {
    pub fn new(r: RoadID, clicked_pt: Pt2D, map: &Map) -> Self {

        let dst_i = map.get_r(r).dst_i;
        let src_i = map.get_r(r).src_i;

        let dst_m = clicked_pt.fast_dist(map.get_i(dst_i).polygon.center());
        let src_m = clicked_pt.fast_dist(map.get_i(src_i).polygon.center());
        
        let i: IntersectionID;
        if dst_m > src_m {
            i = src_i;
        } else {
            i = dst_i;
        }

        let prohibited_t = restricted_destination_roads(map, r, Some(i));
        let permitted_t = destination_roads(map, r, Some(i));

        let mut ft = FocusedTurns {
            src_r: r,
            i,
            hull : Polygon::dummy(),
            permitted_t,
            prohibited_t,
        };

        ft.hull = hull_around_focused_turns(map, r,&ft.permitted_t, &ft.prohibited_t);
        ft
    }
}

fn hull_around_focused_turns(map: &Map, r: RoadID, permitted_t: &HashSet<RoadID>, prohibited_t: &HashSet<RoadID>) -> Polygon {

    let mut all_pt: Vec<Pt2D> = Vec::new();

    all_pt.extend(map.get_r(r).get_thick_polygon().get_outer_ring().clone().into_points());

    // Polygon::concave_hull(points, concavity)
    for t in permitted_t {
        all_pt.extend(map.get_r(*t).get_thick_polygon().get_outer_ring().clone().into_points());
    }

    for t in prohibited_t {
        all_pt.extend(map.get_r(*t).get_thick_polygon().get_outer_ring().clone().into_points());
    }

    // TODO the `200` value seems to work for some cases. But it is arbitary and there is no science
    // behind its the value. Need to work out what is an appropriate value _and why_.
    Polygon::concave_hull(all_pt, 200).unwrap_or(Polygon::dummy())
}

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

/// Returns a HashSet of all roads which are connected by driving from RoadID.
/// This accounts for oneway restrictions, but not turn restrictions. eg:
/// 
/// - If a oneway restriction on either the 'source road' or the 'destination road' would prevent driving from
/// source to destination, then 'destination road' it will NOT be included in the result.
/// - If a turn restriction exists and is the only thing that would prevent driving from 'source road' or the
/// 'destination road', then the 'destination road' will still be included in the result.
/// 
/// `i` is Optional. If `i` is `Some` then, it must be connected to `source_r_id`. It is used to filter
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
            if source_r.id != *r && is_road_drivable_from_i(&map, *r, source_r.src_i){
                destinations.insert(*r);
            }
        }
    }

    if one_way != Some(Direction::Back) && Some(source_r.src_i) != i {
        for r in &map.get_i(source_r.dst_i).roads {
            if source_r.id != *r && is_road_drivable_from_i(&map, *r, source_r.dst_i) {
                destinations.insert(*r);
            }
        }
    }
    destinations
}

fn is_road_drivable_from_i(map: &Map, r: RoadID, i: IntersectionID) -> bool {

    let road = map.get_r(r);
    let one_way = road.oneway_for_driving();
    
    return (road.src_i == i && one_way != Some(Direction::Back)) ||
           (road.dst_i == i && one_way != Some(Direction::Fwd)) 

}

#[cfg(test)]
mod tests {
    use tests::{import_map, get_test_file_path};
    use super::{destination_roads, restricted_destination_roads, FocusedTurns};
    use map_model::{RoadID, IntersectionID};
    use std::collections::HashSet;
    use geom::Pt2D;

    #[test]
    fn test_focused_turn_restriction() -> Result<(), anyhow::Error> {
        // Test that the correct intersection is selected when creating a FocusTurns object

        // Get example map
        let file_name = get_test_file_path(String::from("input/turn_restriction_ltn_boundary.osm"));
        let map = import_map(file_name.unwrap());

        let r = RoadID(11);
        let road = map.get_r(r);
        // south west
        let click_pt_1 = Pt2D::new(192.5633, 215.7847);
        let expected_i_1 = 3;
        // north east 
        let click_pt_2 = Pt2D::new(214.7931, 201.7212);
        let expects_i_2 = 13;

        for (click_pt, i_id) in [
            (click_pt_1, expected_i_1),
            (click_pt_2, expects_i_2)
        ] {
            let ft = FocusedTurns::new(r, click_pt, &map);
            
            println!("ft.i          {:?}", ft.i);
            assert_eq!(ft.i, IntersectionID(i_id));
            assert!([road.src_i, road.dst_i].contains(&ft.i));
        }

        Ok(())
    }


    #[test]
    fn test_destination_roads() -> Result<(), anyhow::Error> {

        // Get example map
        let file_name = get_test_file_path(String::from("input/turn_restriction_ltn_boundary.osm"));
        let map = import_map(file_name.unwrap());

        // hard coded values for "turn_restriction_ltn_boundary"
        let src_r = RoadID(11);
        let src_road = map.get_r(src_r);
        let expected_all_r = vec![3usize, 4, 9, 12].iter().map(|n| RoadID(*n)).collect::<HashSet<_>>();
        let expected_filters_dst_i = vec![9usize, 12].iter().map(|n| RoadID(*n)).collect::<HashSet<_>>();
        let expected_filters_src_i = vec![3usize, 4].iter().map(|n| RoadID(*n)).collect::<HashSet<_>>();

        // Three test cases
        for (i , expected) in [
            (None, expected_all_r),
            (Some(src_road.dst_i), expected_filters_dst_i),
            (Some(src_road.src_i), expected_filters_src_i),
        ] {
            let actual_vec = destination_roads(&map, src_r, i);
            let mut actual = HashSet::<RoadID>::new();
            actual.extend(actual_vec.iter());

            for dst_r in actual.iter() {
                println!("destination_roads, src_r {}, dst_r = {}", src_r, dst_r);
            }
            assert_eq!(actual, expected);
        }
        Ok(())
    }

    #[test]
    fn test_destination_roads_connected_one_ways() -> Result<(), anyhow::Error> {
        struct TurnRestrictionTestCase {
            pub input_file: String,
            pub r: RoadID,
            pub permitted_dst_i: HashSet<RoadID>,
            pub permitted_src_i: HashSet<RoadID>,
            pub prohibited_dst_i: HashSet<RoadID>,
            pub prohibited_src_i: HashSet<RoadID>,
        }
        
        let test_cases = [
            TurnRestrictionTestCase {
                input_file: String::from("input/false_positive_u_turns.osm"),
                // north end is dst according to JOSM
                r: RoadID(5),
                // Can continue on north bound left-hand lane past central barrier
                permitted_dst_i: HashSet::from([RoadID(1)]),
                // Cannot continue on southbound right-hand (opposing oneway) past barrier RoadID(3)
                // but this is already restricted by virtue of being oneway 
                prohibited_dst_i: HashSet::new(),
                // Can continue south onto Tyne bridge (RoadID(0))
                // Right turn would prevent turing onto right on Pilgrim Street North bound (RoadID(2))
                permitted_src_i: HashSet::from([RoadID(2), RoadID(0)]),
                // Cannot turn right onto Pilgrim Street North bound (RoadID(2)).
                // Also cannot go backward up southbound ramp (RoadID(4) which is a oneway.
                prohibited_src_i: HashSet::from([RoadID(2)]),
            },
            TurnRestrictionTestCase {
                input_file: String::from("input/false_positive_u_turns.osm"),
                // north end is src according to JOSM
                r: RoadID(0),
                // Off the edge of the map
                permitted_dst_i: HashSet::new(),
                // Off the edge of the map
                prohibited_dst_i: HashSet::new(),
                // Can continue south onto Tyne bridge
                permitted_src_i: HashSet::from([RoadID(5), RoadID(2)]),
                // Cannot turn right onto Pilgrim Street North bound - Cannot go backward up southbound oneway ramp (RoadID(4))
                prohibited_src_i: HashSet::new(),
            },
        ];

        for tc in test_cases {
            // Get example map
            let file_name = get_test_file_path(tc.input_file.clone());
            let map = import_map(file_name.unwrap());

            // Three combinations of road/intersection for each test case
            for (i , expected_permitted, expected_prohibited) in [
                (Some(map.get_r(tc.r).dst_i), tc.permitted_dst_i, tc.prohibited_dst_i),
                (Some(map.get_r(tc.r).src_i), tc.permitted_src_i, tc.prohibited_src_i),
                // (None,
                //  tc.permitted_dst_i.union(tc.permitted_src_i).collect::<HashSet<_>>(),
                //  tc.prohibited_dst_i.union(tc.prohibited_src_i).collect::<HashSet<_>>()
                // )
            ] {
                let actual_permitted = destination_roads(&map, tc.r, i);
                let actual_prohibited = restricted_destination_roads(&map, tc.r, i);

                println!("r={:?}, i={:?}, file={:?}", &tc.r, i, &tc.input_file);
                for dst_r in actual_permitted.iter() {
                    println!("destination_roads, src_r {}, dst_r = {}", tc.r, dst_r);
                }
                assert_eq!(actual_prohibited, expected_prohibited);
                assert_eq!(actual_permitted, expected_permitted);
            }
        }
        Ok(())
    }


}