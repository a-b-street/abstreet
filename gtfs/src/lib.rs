use geom::LonLat;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use transitfeed::GTFSIterator;

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Route {
    pub name: String,
    pub stops: Vec<LonLat>,
}

pub fn load(dir_path: &str) -> Vec<Route> {
    println!("Loading GTFS from {}", dir_path);

    let mut route_id_to_name: HashMap<String, String> = HashMap::new();
    for rec in GTFSIterator::<_, transitfeed::Route>::from_path(&format!("{}/routes.txt", dir_path))
        .unwrap()
    {
        let rec = rec.unwrap();
        route_id_to_name.insert(rec.route_id.clone(), rec.route_short_name.clone());
    }

    let mut stop_id_to_pt: HashMap<String, LonLat> = HashMap::new();
    for rec in
        GTFSIterator::<_, transitfeed::Stop>::from_path(&format!("{}/stops.txt", dir_path)).unwrap()
    {
        let rec = rec.unwrap();
        stop_id_to_pt.insert(rec.stop_id.clone(), LonLat::new(rec.stop_lon, rec.stop_lat));
    }

    let mut trip_id_to_route_id_and_direction: HashMap<String, (String, bool)> = HashMap::new();
    for rec in
        GTFSIterator::<_, transitfeed::Trip>::from_path(&format!("{}/trips.txt", dir_path)).unwrap()
    {
        let rec = rec.unwrap();
        trip_id_to_route_id_and_direction.insert(
            rec.trip_id.clone(),
            (
                rec.route_id.clone(),
                rec.direction_id.map(|d| d == "0").unwrap_or(true),
            ),
        );
    }

    // Each (directed) route has many trips. Ignore all but the first and assume the list of stops
    // is the same. Also assume that records with the same trip are contiguous and that
    // stop_sequence is monotonic.
    let mut directed_routes: HashMap<(String, bool), Vec<LonLat>> = HashMap::new();
    for (key, group) in
        GTFSIterator::<_, transitfeed::StopTime>::from_path(&format!("{}/stop_times.txt", dir_path))
            .unwrap()
            .map(|rec| rec.unwrap())
            // TODO This only groups records with consecutive same trip ID. Might be a bug.
            .group_by(|rec| rec.trip_id.clone())
            .into_iter()
    {
        let (route_id, forwards) = trip_id_to_route_id_and_direction[&key].clone();
        if directed_routes.contains_key(&(route_id.clone(), forwards)) {
            continue;
        }
        directed_routes.insert(
            (route_id, forwards),
            group.map(|rec| stop_id_to_pt[&rec.stop_id]).collect(),
        );
    }

    // Group together the pairs of directed routes
    let route_ids: BTreeSet<String> = directed_routes
        .keys()
        .map(|(id, _)| id.to_string())
        .collect();
    let mut results = Vec::new();
    for route_id in route_ids {
        let mut stops = directed_routes
            .remove(&(route_id.clone(), true))
            .unwrap_or_else(Vec::new);
        if let Some(more_stops) = directed_routes.remove(&(route_id.clone(), false)) {
            stops.extend(more_stops);
        }
        assert!(!stops.is_empty());
        results.push(Route {
            name: route_id_to_name[&route_id].to_string(),
            stops,
        });
    }
    assert!(directed_routes.is_empty());

    results
}
