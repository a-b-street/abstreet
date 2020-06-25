use crate::make::sidewalk_finder::find_sidewalk_points;
use crate::{
    BusRoute, BusRouteID, BusStop, BusStopID, LaneID, LaneType, Map, PathConstraints, PathRequest,
    Position,
};
use abstutil::{MultiMap, Timer};
use geom::{Bounds, Distance, GPSBounds, HashablePt2D, Pt2D};
use gtfs;
use std::collections::{BTreeMap, HashMap, HashSet};

pub fn make_bus_stops(
    map: &Map,
    bus_routes: &Vec<gtfs::Route>,
    gps_bounds: &GPSBounds,
    bounds: &Bounds,
    timer: &mut Timer,
) -> (BTreeMap<BusStopID, BusStop>, Vec<BusRoute>) {
    timer.start("make bus stops");
    let mut bus_stop_pts: HashSet<HashablePt2D> = HashSet::new();
    let mut route_lookups: HashMap<String, Vec<HashablePt2D>> = HashMap::new();
    for route in bus_routes {
        for gps in &route.stops {
            if let Some(pt) = Pt2D::from_gps(*gps, gps_bounds) {
                let hash_pt = pt.to_hashable();
                bus_stop_pts.insert(hash_pt);
                route_lookups
                    .entry(route.name.clone())
                    .or_insert_with(Vec::new)
                    .push(hash_pt);
            }
        }
    }

    let mut stops_per_sidewalk: MultiMap<LaneID, (Distance, HashablePt2D)> = MultiMap::new();
    for (pt, pos) in find_sidewalk_points(
        bounds,
        bus_stop_pts,
        map.all_lanes(),
        Distance::ZERO,
        Distance::meters(10.0),
        timer,
    )
    .into_iter()
    {
        stops_per_sidewalk.insert(pos.lane(), (pos.dist_along(), pt));
    }
    let mut point_to_stop_id: HashMap<HashablePt2D, BusStopID> = HashMap::new();
    let mut bus_stops: BTreeMap<BusStopID, BusStop> = BTreeMap::new();

    for (sidewalk_id, dists_set) in stops_per_sidewalk.consume().into_iter() {
        let road = map.get_parent(sidewalk_id);
        if let Ok(driving_lane) =
            road.find_closest_lane(sidewalk_id, vec![LaneType::Driving, LaneType::Bus])
        {
            let mut dists: Vec<(Distance, HashablePt2D)> = dists_set.into_iter().collect();
            dists.sort_by_key(|(dist, _)| *dist);
            for (idx, (dist_along, orig_pt)) in dists.into_iter().enumerate() {
                let stop_id = BusStopID {
                    sidewalk: sidewalk_id,
                    idx,
                };
                point_to_stop_id.insert(orig_pt, stop_id);
                let sidewalk_pos = Position::new(sidewalk_id, dist_along);
                let driving_pos = sidewalk_pos.equiv_pos(driving_lane, Distance::ZERO, map);
                bus_stops.insert(
                    stop_id,
                    BusStop {
                        id: stop_id,
                        sidewalk_pos,
                        driving_pos,
                    },
                );
            }
        } else {
            timer.warn(format!(
                "Can't find driving lane next to {}: {:?} and {:?}",
                sidewalk_id, road.children_forwards, road.children_backwards
            ));
        }
    }

    let mut routes: Vec<BusRoute> = Vec::new();
    for route in bus_routes {
        let route_name = route.name.to_string();
        let stops: Vec<BusStopID> = route_lookups
            .remove(&route_name)
            .unwrap_or_else(Vec::new)
            .into_iter()
            .filter_map(|pt| point_to_stop_id.get(&pt))
            .cloned()
            .collect();
        let id = BusRouteID(routes.len());
        routes.push(BusRoute {
            id,
            name: route_name.to_string(),
            stops,
        });
    }
    timer.stop("make bus stops");
    (bus_stops, routes)
}

pub fn fix_bus_route(map: &Map, r: &mut BusRoute) -> bool {
    // Trim out stops if needed; map borders sometimes mean some paths don't work.
    let mut stops = Vec::new();
    for stop in r.stops.drain(..) {
        if stops.is_empty() {
            stops.push(stop);
        } else {
            if check_stops(*stops.last().unwrap(), stop, map) {
                stops.push(stop);
            }
        }
    }
    // Don't forget the last and first
    while stops.len() >= 2 {
        if check_stops(*stops.last().unwrap(), stops[0], map) {
            break;
        }
        // TODO Or the front one
        stops.pop();
    }
    r.stops = stops;
    r.stops.len() >= 2
}

fn check_stops(stop1: BusStopID, stop2: BusStopID, map: &Map) -> bool {
    let bs1 = map.get_bs(stop1);
    let bs2 = map.get_bs(stop2);
    // This is coming up because the dist_along's are in a bad order. But why should
    // this happen at all?
    let ok1 = bs1.driving_pos.lane() != bs2.driving_pos.lane();
    let ok2 = map
        .pathfind(PathRequest {
            start: bs1.driving_pos,
            end: bs2.driving_pos,
            constraints: PathConstraints::Bus,
        })
        .is_some();
    ok1 && ok2
}
