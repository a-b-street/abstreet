use dimensioned::si;
use geom::{Bounds, HashablePt2D, Pt2D};
use gtfs;
use make::sidewalk_finder::find_sidewalk_points;
use multimap::MultiMap;
use ordered_float::NotNaN;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::iter;
use {BusRoute, BusStop, BusStopID, Lane, LaneID, Map, Pathfinder, Road};

pub fn make_bus_stops(
    lanes: &mut Vec<Lane>,
    roads: &Vec<Road>,
    bus_routes: &Vec<gtfs::Route>,
    bounds: &Bounds,
) -> (BTreeMap<BusStopID, BusStop>, Vec<BusRoute>) {
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
    let mut point_to_stop_id: HashMap<HashablePt2D, BusStopID> = HashMap::new();
    let mut bus_stops: BTreeMap<BusStopID, BusStop> = BTreeMap::new();

    for (id, dists) in stops_per_sidewalk.iter_all_mut() {
        let road = &roads[lanes[id.0].parent.0];
        if let Ok(driving_lane) = road.find_driving_lane_from_sidewalk(*id) {
            dists.sort_by_key(|(dist, _)| NotNaN::new(dist.value_unsafe).unwrap());
            for (idx, (dist_along, orig_pt)) in dists.iter().enumerate() {
                let stop_id = BusStopID { sidewalk: *id, idx };
                point_to_stop_id.insert(*orig_pt, stop_id);
                lanes[id.0].bus_stops.push(stop_id);
                bus_stops.insert(
                    stop_id,
                    BusStop {
                        id: stop_id,
                        driving_lane,
                        dist_along: *dist_along,
                    },
                );
            }
        } else {
            warn!(
                "Can't find driving lane next to {}: {:?} and {:?}",
                id, road.children_forwards, road.children_backwards
            );
        }
    }

    let mut routes: Vec<BusRoute> = Vec::new();
    for (route_name, stop_points) in route_lookups.iter_all() {
        let stops: Vec<BusStopID> = stop_points
            .iter()
            .filter_map(|pt| point_to_stop_id.get(pt))
            .map(|stop| *stop)
            .collect();
        if stops.len() == 1 {
            warn!(
                "Skipping route {} since it only has 1 stop in the slice of the map",
                route_name
            );
            continue;
        }
        routes.push(BusRoute {
            name: route_name.to_string(),
            stops,
        });
    }
    (bus_stops, routes)
}

pub fn verify_bus_routes(map: &Map, routes: Vec<BusRoute>) -> Vec<BusRoute> {
    routes
        .into_iter()
        .filter(|r| {
            let mut ok = true;
            for (stop1, stop2) in r
                .stops
                .iter()
                .zip(r.stops.iter().skip(1))
                .chain(iter::once((r.stops.last().unwrap(), &r.stops[0])))
            {
                let bs1 = map.get_bs(*stop1);
                let bs2 = map.get_bs(*stop2);
                if Pathfinder::shortest_distance(map, bs1.driving_lane, bs2.driving_lane).is_none()
                {
                    warn!(
                        "Removing route {} since {:?} and {:?} aren't connected",
                        r.name, bs1, bs2
                    );
                    ok = false;
                    break;
                }
            }
            ok
        }).collect()
}
