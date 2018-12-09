use abstutil::elapsed_seconds;
use failure::Error;
use geom::LonLat;
use itertools::Itertools;
use serde_derive::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
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

    let mut trip_id_to_route_id: HashMap<String, String> = HashMap::new();
    for rec in csv::Reader::from_reader(File::open(format!("{}/trips.txt", dir_path))?).records() {
        let rec = rec?;
        trip_id_to_route_id.insert(rec[2].to_string(), rec[0].to_string());
    }

    // Each route has many trips. Ignore all but the first and assume the list of stops is the
    // same. Also assume that records with the same trip are contiguous and that stop_sequence is
    // monotonic.
    let mut route_ids_used: HashSet<String> = HashSet::new();
    let mut results: Vec<Route> = Vec::new();

    let mut reader = csv::Reader::from_reader(File::open(format!("{}/stop_times.txt", dir_path))?);
    for (key, group) in reader
        .records()
        .group_by(|rec| rec.as_ref().unwrap()[0].to_string())
        .into_iter()
    {
        let route_id = trip_id_to_route_id[&key].to_string();
        if route_ids_used.contains(&route_id) {
            continue;
        }
        route_ids_used.insert(route_id.clone());

        results.push(Route {
            name: route_id_to_name[&route_id].to_string(),
            stops: group.map(|rec| stop_id_to_pt[&rec.unwrap()[3]]).collect(),
        });
    }

    println!("Loading GTFS took {}s", elapsed_seconds(timer));
    Ok(results)
}
