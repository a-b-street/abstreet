use std::collections::HashSet;

use map_model::{Map, RoadID, IntersectionID};
use osm2streets::{Direction, RestrictionType};
use geom::{Polygon, Pt2D};

/// An attempt to standardise language around turn restrictions.
/// NOTE AT PRESENT THIS IS ASPIRATIONAL - DO NOT ASSUME THAT THE RELEVANT CODE ADHERES TO THESE RULES
/// Future refactoring should migrate to these conventions.
///
/// Summary:
/// -------
/// ```notrust
///     {connected} == { permitted ∪ opposing_oneway ∪ restricted_turn }
/// ```
/// 
/// Details:
/// ------- 
/// When moving (or attempting to move) from "RoadA" to "RoadB" the follow terms should be used:
/// "from_r"    = RoadA
/// "target_r"  = RoadB
/// "connected" = RoadB will be a member of "connected" if RoadB share a common intersection RoadA, or 
///               is part of a shared complex turn with RoadA. The legality of driving from RoadA to RoadB
///               is not a concern for "connected". "Connected" is the superset of all the other categories
///               listed here.
/// "permitted" = RoadB is a member of "permitted", if RoadB is a member of "connected" and it is legal to 
///               drive from RoadA to RoadB. Lane-level restrictions are not considered, so as long as some
///               route from one or more driving Lanes in RoadA to one or more Lanes in RoadB then RoadB is 
///               considered "permitted".
/// "opposing_oneways" = RoadB is oneway for driving, and driving from RoadA to RoadB would result in driving the
///                     wrong way along RoadB.
/// "restricted_turns" = RoadB will be a member of "restricted_turns" if
///                         a) RoadB is a member of "connected"
///                         b) There is explicitly tagged turn restriction which prohibits traffic turning from
///                            RoadA to RoadB, OR there is an explicitly tagged turn restriction which mandates
///                            traffic from RoadA must turn onto a different road to RoadB.
///                         c) RoadB is not a member of "opposing_oneways"
/// possible_turns = { connected - opposing_oneways } == { permitted + restricted_turns }
///                  These are turns that would be possible if all turn restrictions where removed.
/// 
/// Notes:
/// -----
/// * RoadA will NOT be a member of any of the groups connected, permitted, opposing_oneway, restricted_turn
///   even if a no U-turns restriction exists
/// * In reality a road/turn maybe signposted by both turn restrictions and oneway restrictions.
///   Following (OSM practise)[https://wiki.openstreetmap.org/wiki/Relation:restriction#When_to_map] it is not 
///   necessary mark turn restrictions when they are already implied by opposing oneway restrictions. We treat 
///   "banned_turn" and "opposing_oneway" as mutually exclusive. 
///
/// Discouraged terms:
/// -----------------
/// "prohibited_turn" = use "restricted_turn" instead.
/// "banned_turns" = use "restricted_turn" instead where practical. "Banned" is used elsewhere in A/BStreet,
///                  (ie `RestrictionType::BanTurns`) but within the LTN tool "restricted" is preferred, as it
///                  is more consistent with the `road.restricted_turns` and `road.complicated_turn_restrictions`
///                  as well the general OSM tagging.
/// "src_r" and "dst_r" = use "from_r" and "target_r" instead. ("src_r" and "dst_r" are too similar to
///                       `road.src_i` and `road.dst_i` which are conceptually very different).
pub struct FocusedTurns {
    pub from_r: RoadID,
    pub i: IntersectionID,
    pub hull: Polygon,
    pub possible_t: HashSet<RoadID>,
    pub restricted_t: HashSet<RoadID>,
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

        let restricted_t = restricted_destination_roads(map, r, Some(i));
        let possible_t = possible_destination_roads(map, r, Some(i));

        let mut ft = FocusedTurns {
            from_r: r,
            i,
            hull : Polygon::dummy(),
            possible_t,
            restricted_t,
        };

        ft.hull = hull_around_focused_turns(map, r, &ft.possible_t, &ft.restricted_t);
        ft
    }
}

fn hull_around_focused_turns(map: &Map, r: RoadID, permitted_t: &HashSet<RoadID>, restricted_t: &HashSet<RoadID>) -> Polygon {

    let mut all_pt: Vec<Pt2D> = Vec::new();

    all_pt.extend(map.get_r(r).get_thick_polygon().get_outer_ring().clone().into_points());

    // Polygon::concave_hull(points, concavity)
    for t in permitted_t {
        all_pt.extend(map.get_r(*t).get_thick_polygon().get_outer_ring().clone().into_points());
    }

    for t in restricted_t {
        all_pt.extend(map.get_r(*t).get_thick_polygon().get_outer_ring().clone().into_points());
    }

    // TODO the `200` value seems to work for some cases. But it is arbitrary and there is no science
    // behind its the value. Need to work out what is an appropriate value _and why_.
    Polygon::concave_hull(all_pt, 200).unwrap_or(Polygon::dummy())
}

/// Returns all roads that are possible destinations from the given "from_road" where the turn is currently
/// prohibited by a turn restriction.
pub fn restricted_destination_roads(map: &Map, from_road_id: RoadID, i: Option<IntersectionID>) -> HashSet<RoadID> {
    let candidate_roads = possible_destination_roads(map, from_road_id, i);

    let from_road = map.get_r(from_road_id);
    let mut restricted_destinations: HashSet<RoadID> = HashSet::new();
        
    for (restriction, r2) in &from_road.turn_restrictions {
        if *restriction == RestrictionType::BanTurns && candidate_roads.contains(r2) {
            restricted_destinations.insert(*r2);
        }
    }
    for (via, r2) in &from_road.complicated_turn_restrictions {
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
/// 
// TODO highlighting possible destinations for complicated turns (at present both sections of existing
// complicated_turn_restrictions are included). However possible future complicated turns are not detected.
//
// TODO Rework `possible_destination_roads()` and `restricted_destination_roads()` to a extra function that
// returns a tupple `(permitted, opposing_oneway, restricted_turn)`
pub fn possible_destination_roads(map: &Map, from_r: RoadID, i: Option<IntersectionID>) -> HashSet<RoadID> {

    if i.is_some() && !verify_intersection(map, from_r, i.unwrap()){
        panic!("IntersectionID {:?}, does not connect to RoadID {:?}", i, from_r);
    }

    let from_road = map.get_r(from_r);
    let mut target_roads: HashSet<RoadID> = HashSet::new();

    let one_way = from_road.oneway_for_driving();

    if one_way != Some(Direction::Fwd) && Some(from_road.dst_i) != i {
        for r in &map.get_i(from_road.src_i).roads {
            if from_road.id != *r && is_road_drivable_from_i(&map, *r, from_road.src_i){
                target_roads.insert(*r);
            }
        }
    }

    if one_way != Some(Direction::Back) && Some(from_road.src_i) != i {
        for r in &map.get_i(from_road.dst_i).roads {
            if from_road.id != *r && is_road_drivable_from_i(&map, *r, from_road.dst_i) {
                target_roads.insert(*r);
            }
        }
    }
    target_roads
}

fn is_road_drivable_from_i(map: &Map, target_r: RoadID, i: IntersectionID) -> bool {

    let road = map.get_r(target_r);
    let one_way = road.oneway_for_driving();
    
    return (road.src_i == i && one_way != Some(Direction::Back)) ||
           (road.dst_i == i && one_way != Some(Direction::Fwd)) 

}

#[cfg(test)]
mod tests {
    use tests::{import_map, get_test_file_path};
    use super::{possible_destination_roads, restricted_destination_roads, FocusedTurns};
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
        let from_r = RoadID(11);
        let from_road = map.get_r(from_r);
        // Expected possible turns for either intersection
        let expected_possible_all_r = vec![3usize, 4, 9, 12].iter().map(|n| RoadID(*n)).collect::<HashSet<_>>();
        // Expected possible turns via `from_r.dst_i`
        let expected_possible_for_dst_i = vec![9usize, 12].iter().map(|n| RoadID(*n)).collect::<HashSet<_>>();
        // Expected possible turns via `from_r.src_i`
        let expected_possible_for_src_i = vec![3usize, 4].iter().map(|n| RoadID(*n)).collect::<HashSet<_>>();

        // Three test cases
        for (i , expected) in [
            (None, expected_possible_all_r),
            (Some(from_road.dst_i), expected_possible_for_dst_i),
            (Some(from_road.src_i), expected_possible_for_src_i),
        ] {
            let actual_vec = possible_destination_roads(&map, from_r, i);
            let mut actual = HashSet::<RoadID>::new();
            actual.extend(actual_vec.iter());

            for target_r in actual.iter() {
                println!("destination_roads, src_r {}, dst_r = {}", from_r, target_r);
            }
            assert_eq!(actual, expected);
        }
        Ok(())
    }

    #[test]
    fn test_destination_roads_connected_one_ways() -> Result<(), anyhow::Error> {
        struct TurnRestrictionTestCase {
            pub input_file: String,
            pub from_r: RoadID,
            pub possible_for_dst_i: HashSet<RoadID>,
            pub possible_for_src_i: HashSet<RoadID>,
            pub restricted_for_dst_i: HashSet<RoadID>,
            pub restricted_for_src_i: HashSet<RoadID>,
        }
        
        let test_cases = [
            TurnRestrictionTestCase {
                input_file: String::from("input/false_positive_u_turns.osm"),
                // north end is dst according to JOSM
                from_r: RoadID(5),
                // Can continue on north bound left-hand lane past central barrier
                possible_for_dst_i: HashSet::from([RoadID(1)]),
                // Cannot continue on southbound right-hand (opposing oneway) past barrier RoadID(3)
                // but this is already restricted by virtue of being oneway 
                restricted_for_dst_i: HashSet::new(),
                // Can continue south onto Tyne bridge (RoadID(0))
                // Right turn would prevent turing onto right on Pilgrim Street North bound (RoadID(2))
                possible_for_src_i: HashSet::from([RoadID(2), RoadID(0)]),
                // Cannot turn right onto Pilgrim Street North bound (RoadID(2)).
                // Also cannot go backward up southbound ramp (RoadID(4) which is a oneway.
                restricted_for_src_i: HashSet::from([RoadID(2)]),
            },
            TurnRestrictionTestCase {
                input_file: String::from("input/false_positive_u_turns.osm"),
                // north end is src according to JOSM
                from_r: RoadID(0),
                // Off the edge of the map
                possible_for_dst_i: HashSet::new(),
                // Off the edge of the map
                restricted_for_dst_i: HashSet::new(),
                // Can continue south onto Tyne bridge
                possible_for_src_i: HashSet::from([RoadID(5), RoadID(2)]),
                // Cannot turn right onto Pilgrim Street North bound - Cannot go backward up southbound oneway ramp (RoadID(4))
                restricted_for_src_i: HashSet::new(),
            },
        ];

        for tc in test_cases {
            // Get example map
            let file_name = get_test_file_path(tc.input_file.clone());
            let map = import_map(file_name.unwrap());

            // Three combinations of road/intersection for each test case
            for (i , expected_possible, expected_restricted) in [
                (Some(map.get_r(tc.from_r).dst_i), tc.possible_for_dst_i, tc.restricted_for_dst_i),
                (Some(map.get_r(tc.from_r).src_i), tc.possible_for_src_i, tc.restricted_for_src_i),
                // (None,
                //  tc.permitted_dst_i.union(tc.permitted_src_i).collect::<HashSet<_>>(),
                //  tc.prohibited_dst_i.union(tc.prohibited_src_i).collect::<HashSet<_>>()
                // )
            ] {
                let actual_possible = possible_destination_roads(&map, tc.from_r, i);
                let actual_restricted = restricted_destination_roads(&map, tc.from_r, i);

                println!("r={:?}, i={:?}, file={:?}", &tc.from_r, i, &tc.input_file);
                for target_r in actual_possible.iter() {
                    println!("destination_roads, src_r {}, dst_r = {}", tc.from_r, target_r);
                }
                assert_eq!(actual_restricted, expected_restricted);
                assert_eq!(actual_possible, expected_possible);
            }
        }
        Ok(())
    }


}