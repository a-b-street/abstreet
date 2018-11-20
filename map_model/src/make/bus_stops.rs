use abstutil::Timer;
use dimensioned::si;
use geom::{Bounds, GPSBounds, HashablePt2D, Pt2D};
use gtfs;
use make::sidewalk_finder::find_sidewalk_points;
use multimap::MultiMap;
use ordered_float::NotNaN;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::iter;
use {BusRoute, BusStop, BusStopID, Lane, LaneID, LaneType, Map, PathRequest, Pathfinder, Road};

pub fn make_bus_stops(
    lanes: &mut Vec<Lane>,
    roads: &Vec<Road>,
    bus_routes: &Vec<gtfs::Route>,
    gps_bounds: &GPSBounds,
    bounds: &Bounds,
    timer: &mut Timer,
) -> (BTreeMap<BusStopID, BusStop>, Vec<BusRoute>) {
    timer.start("make bus stops");
    let mut bus_stop_pts: HashSet<HashablePt2D> = HashSet::new();
    let mut route_lookups: MultiMap<String, HashablePt2D> = MultiMap::new();
    for route in bus_routes {
        for gps in &route.stops {
            if let Some(pt) = Pt2D::from_gps(*gps, gps_bounds) {
                let hash_pt: HashablePt2D = pt.into();
                bus_stop_pts.insert(hash_pt);
                route_lookups.insert(route.name.to_string(), hash_pt);
            }
        }
    }

    let mut stops_per_sidewalk: MultiMap<LaneID, (si::Meter<f64>, HashablePt2D)> = MultiMap::new();
    for (pt, (lane, dist_along)) in
        find_sidewalk_points(bounds, bus_stop_pts, lanes, 10.0 * si::M, timer).iter()
    {
        stops_per_sidewalk.insert(*lane, (*dist_along, *pt));
    }
    let mut point_to_stop_id: HashMap<HashablePt2D, BusStopID> = HashMap::new();
    let mut bus_stops: BTreeMap<BusStopID, BusStop> = BTreeMap::new();

    for (id, dists) in stops_per_sidewalk.iter_all_mut() {
        let road = &roads[lanes[id.0].parent.0];
        if let Ok(driving_lane) =
            road.find_closest_lane(*id, vec![LaneType::Driving, LaneType::Bus])
        {
            let driving_len = lanes[driving_lane.0].length();
            dists.sort_by_key(|(dist, _)| NotNaN::new(dist.value_unsafe).unwrap());
            for (idx, (dist_along, orig_pt)) in dists.iter().enumerate() {
                // TODO Should project perpendicular line to find equivalent dist_along for
                // different lanes. Till then, just skip this.
                if *dist_along > driving_len {
                    warn!(
                        "Skipping bus stop at {} along {}, because driving lane {} is only {} long",
                        dist_along, id, driving_lane, driving_len
                    );
                    continue;
                }

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
    for route in bus_routes {
        let route_name = route.name.to_string();
        if let Some(stop_points) = route_lookups.get_vec(&route_name) {
            let stops: Vec<BusStopID> = stop_points
                .iter()
                .filter_map(|pt| point_to_stop_id.get(pt))
                .map(|stop| *stop)
                .collect();
            if stops.len() < 2 {
                warn!(
                    "Skipping route {} since it only has {} stop in the slice of the map",
                    route_name,
                    stops.len()
                );
                continue;
            }
            routes.push(BusRoute {
                name: route_name.to_string(),
                stops,
            });
        }
    }
    timer.stop("make bus stops");
    (bus_stops, routes)
}

pub fn verify_bus_routes(map: &Map, routes: Vec<BusRoute>, timer: &mut Timer) -> Vec<BusRoute> {
    timer.start_iter("verify bus routes are connected", routes.len());
    routes
        .into_iter()
        .filter(|r| {
            timer.next();
            let mut ok = true;
            for (stop1, stop2) in r
                .stops
                .iter()
                .zip(r.stops.iter().skip(1))
                .chain(iter::once((r.stops.last().unwrap(), &r.stops[0])))
            {
                let bs1 = map.get_bs(*stop1);
                let bs2 = map.get_bs(*stop2);
                if bs1.driving_lane == bs2.driving_lane {
                    // This is coming up because the dist_along's are in a bad order. But why
                    // should this happen at all?
                    warn!(
                        "Removing route {} since {:?} and {:?} are on the same lane",
                        r.name, bs1, bs2
                    );
                    ok = false;
                    break;
                }

                if Pathfinder::shortest_distance(
                    map,
                    PathRequest {
                        start: bs1.driving_lane,
                        start_dist: bs1.dist_along,
                        end: bs2.driving_lane,
                        end_dist: bs2.dist_along,
                        can_use_bike_lanes: false,
                        can_use_bus_lanes: true,
                    },
                ).is_none()
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
