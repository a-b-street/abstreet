extern crate csv;
extern crate failure;
extern crate geom;

use failure::Error;
use geom::LonLat;
use std::collections::{HashSet, HashMap};
use std::fs::File;
use std::time::Instant;

#[derive(Debug)]
pub struct Route {
    name: String,
    stops: Vec<LonLat>,
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
    // same.
    let mut route_ids_used: HashSet<String> = HashSet::new();
    let mut result: Vec<Route> = Vec::new();
    let mut current_trip_id: Option<String> = None;
    let mut current_stop_ids: Vec<String> = Vec::new();
    for rec in csv::Reader::from_reader(File::open(format!("{}/stop_times.txt", dir_path))?).records() {
        let rec = rec?;
        // Assume the records are contiguous -- records for one trip are contiguous and sorted by
        // stop_sequence already.
        if let Some(trip) = current_trip_id.clone() {
            if rec[0].to_string() == *trip {
                current_stop_ids.push(rec[3].to_string());
            } else {
                // Save the current route?
                let route_id = trip_id_to_route_id[&rec[0]].clone();
                if !route_ids_used.contains(&route_id) {
                    result.push(Route {
                        name: route_id_to_name[&route_id].clone(),
                        stops: current_stop_ids.iter().map(|stop_id| stop_id_to_pt[stop_id]).collect(),
                    });
                    route_ids_used.insert(route_id);
                }

                // Reset for the next trip
                current_trip_id = Some(rec[0].to_string());
                current_stop_ids = vec![rec[3].to_string()];
            }
        } else {
            current_trip_id = Some(rec[0].to_string());
            current_stop_ids.push(rec[3].to_string());
        }
    }
    // Handle the last one. TODO duplicates saving code :(
    let last_route_id = trip_id_to_route_id[&current_trip_id.unwrap()].clone();
    if !route_ids_used.contains(&last_route_id) {
        result.push(Route {
            name: route_id_to_name[&last_route_id].clone(),
            stops: current_stop_ids.iter().map(|stop_id| stop_id_to_pt[stop_id]).collect(),
        });
    }

    let elapsed = timer.elapsed();
    let dt = elapsed.as_secs() as f64 + f64::from(elapsed.subsec_nanos()) * 1e-9;
    println!("Loading GTFS took {}s", dt);
    Ok(result)
}
