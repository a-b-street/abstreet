use dimensioned::si;
use geom::{Bounds, HashablePt2D, Pt2D};
use gtfs;
use make::sidewalk_finder::find_sidewalk_points;
use multimap::MultiMap;
use ordered_float::NotNaN;
use std::collections::{HashMap, HashSet};
use {BusRoute, BusStop, BusStopDetails, Lane, LaneID, Road};

pub fn make_bus_stops(
    lanes: &mut Vec<Lane>,
    roads: &Vec<Road>,
    bus_routes: &Vec<gtfs::Route>,
    bounds: &Bounds,
) -> Vec<BusRoute> {
    let mut bus_stop_pts: HashSet<HashablePt2D> = HashSet::new();
    let mut route_lookups: MultiMap<String, HashablePt2D> = MultiMap::new();
    for route in bus_routes {
        for gps in &route.stops {
            if bounds.contains(gps.longitude, gps.latitude) {
                let pt: HashablePt2D = Pt2D::from_gps(&gps, bounds).into();
                bus_stop_pts.insert(pt);
                route_lookups.insert(route.name.to_string(), pt);
            }
        }
    }

    let mut stops_per_sidewalk: MultiMap<LaneID, (si::Meter<f64>, HashablePt2D)> = MultiMap::new();
    for (pt, (lane, dist_along)) in find_sidewalk_points(bus_stop_pts, lanes).iter() {
        stops_per_sidewalk.insert(*lane, (*dist_along, *pt));
    }
    let mut point_to_stop_idx: HashMap<HashablePt2D, BusStop> = HashMap::new();
    for (id, dists) in stops_per_sidewalk.iter_all_mut() {
        let road = &roads[lanes[id.0].parent.0];
        let driving_lane = road.find_driving_lane_from_sidewalk(*id).expect(&format!(
            "Can't find driving lane next to {}: {:?} and {:?}",
            id, road.children_forwards, road.children_backwards,
        ));
        dists.sort_by_key(|(dist, _)| NotNaN::new(dist.value_unsafe).unwrap());
        for (idx, (dist_along, orig_pt)) in dists.iter().enumerate() {
            let stop_id = BusStop { sidewalk: *id, idx };
            point_to_stop_idx.insert(*orig_pt, stop_id);
            lanes[id.0].bus_stops.push(BusStopDetails {
                id: stop_id,
                driving_lane,
                dist_along: *dist_along,
            });
        }
    }

    let mut results: Vec<BusRoute> = Vec::new();
    for (route_name, stop_points) in route_lookups.iter_all() {
        let stops: Vec<BusStop> = stop_points.iter().map(|pt| point_to_stop_idx[pt]).collect();
        if stops.len() == 1 {
            //println!("Skipping route {} since it only has 1 stop in the slice of the map", route_name);
            continue;
        }
        results.push(BusRoute {
            name: route_name.to_string(),
            stops,
        });
    }
    results
}
