use dimensioned::si;
use geom::{Bounds, HashablePt2D, Pt2D};
use gtfs;
use make::sidewalk_finder::find_sidewalk_points;
use multimap::MultiMap;
use ordered_float::NotNaN;
use std::collections::HashSet;
use {BusStop, BusStopDetails, Lane, LaneID, Road};

pub fn make_bus_stops(
    lanes: &mut Vec<Lane>,
    roads: &Vec<Road>,
    bus_routes: &Vec<gtfs::Route>,
    bounds: &Bounds,
) {
    let mut bus_stop_pts: HashSet<HashablePt2D> = HashSet::new();
    for route in bus_routes {
        for gps in &route.stops {
            if bounds.contains(gps.longitude, gps.latitude) {
                bus_stop_pts.insert(Pt2D::from_gps(&gps, bounds).into());
            }
        }
    }
    println!(
        "Matching {} bus stops to sidewalks, from {} routes",
        bus_stop_pts.len(),
        bus_routes.len()
    );

    let mut stops_per_sidewalk: MultiMap<LaneID, si::Meter<f64>> = MultiMap::new();
    for (lane, dist_along) in find_sidewalk_points(bus_stop_pts, lanes).values() {
        stops_per_sidewalk.insert(*lane, *dist_along);
    }
    for (id, dists) in stops_per_sidewalk.iter_all_mut() {
        // TODO duplicate a little logic from map, and also, this is fragile :)
        let road = &roads[lanes[id.0].parent.0];
        let parking = road.find_parking_lane(*id).unwrap();
        let driving_lane = road.find_driving_lane(parking).unwrap();

        dists.sort_by_key(|dist| NotNaN::new(dist.value_unsafe).unwrap());
        for (idx, dist_along) in dists.iter().enumerate() {
            lanes[id.0].bus_stops.push(BusStopDetails {
                id: BusStop { sidewalk: *id, idx },
                driving_lane,
                dist_along: *dist_along,
            });
        }
    }
}
