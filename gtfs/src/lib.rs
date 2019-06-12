use abstutil::elapsed_seconds;
use failure::Error;
use geom::LonLat;
use itertools::Itertools;
use serde_derive::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use std::fs::File;
use std::time::Instant;

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Route {
    pub name: String,
    pub stops: Vec<LonLat>,
}

pub fn load(dir_path: &str) -> Result<Vec<Route>, Error> {
    println!("Loading GTFS from {}", dir_path);
    let timer = Instant::now();

    let mut route_id_to_name: HashMap<String, String> = HashMap::new();
    for rec in csv::Reader::from_reader(File::open(format!("{}/routes.txt", dir_path))?).records() {
        let rec = rec?;
        route_id_to_name.insert(rec[0].to_string(), rec[2].to_string());
    }

    let mut stop_id_to_pt: HashMap<String, LonLat> = HashMap::new();
    for rec in csv::Reader::from_reader(File::open(format!("{}/stops.txt", dir_path))?).records() {
        let rec = rec?;
        let lon: f64 = rec[5].parse()?;
        let lat: f64 = rec[4].parse()?;
        stop_id_to_pt.insert(rec[0].to_string(), LonLat::new(lon, lat));
    }

    let mut trip_id_to_route_id_and_direction: HashMap<String, (String, bool)> = HashMap::new();
    for rec in csv::Reader::from_reader(File::open(format!("{}/trips.txt", dir_path))?).records() {
        let rec = rec?;
        trip_id_to_route_id_and_direction
            .insert(rec[2].to_string(), (rec[0].to_string(), &rec[5] == "0"));
    }

    // Each (directed) route has many trips. Ignore all but the first and assume the list of stops
    // is the same. Also assume that records with the same trip are contiguous and that
    // stop_sequence is monotonic.
    let mut directed_routes: HashMap<(String, bool), Vec<LonLat>> = HashMap::new();
    let mut reader = csv::Reader::from_reader(File::open(format!("{}/stop_times.txt", dir_path))?);
    for (key, group) in reader
        .records()
        .group_by(|rec| rec.as_ref().unwrap()[0].to_string())
        .into_iter()
    {
        let (route_id, forwards) = trip_id_to_route_id_and_direction[&key].clone();
        if directed_routes.contains_key(&(route_id.clone(), forwards)) {
            continue;
        }
        directed_routes.insert(
            (route_id, forwards),
            group.map(|rec| stop_id_to_pt[&rec.unwrap()[3]]).collect(),
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

    println!("Loading GTFS took {}s", elapsed_seconds(timer));
    Ok(results)
}
