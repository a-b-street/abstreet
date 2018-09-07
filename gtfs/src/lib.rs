extern crate csv;
extern crate failure;
extern crate geom;

use failure::Error;
use geom::LonLat;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::time::Instant;

#[derive(Debug)]
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
    // same.
    let mut route_ids_used: HashSet<String> = HashSet::new();
    let mut results: Vec<Route> = Vec::new();

    // TODO This isn't simple or fast. :(
    // Try implementing an iterator that groups adjacent records matching a predicate.
    let mut reader = csv::Reader::from_reader(File::open(format!("{}/stop_times.txt", dir_path))?);
    let mut iter = reader.records();
    let mut records: Vec<csv::StringRecord> = Vec::new();
    loop {
        if let Some(rec) = iter.next() {
            records.push(rec?);
        } else {
            // We shouldn't have 1 record from next_rec, because a trip shouldn't have just one
            // stop.
            assert!(records.is_empty());
            break;
        }

        let route_id = trip_id_to_route_id[&records[0][0]].to_string();
        let keep_records = !route_ids_used.contains(&route_id);

        // Slurp all records with the same trip ID. Assume they're contiguous.
        let mut next_rec: Option<csv::StringRecord> = None;
        loop {
            if let Some(rec) = iter.next() {
                let rec = rec?;
                if records[0][0] == rec[0] {
                    if keep_records {
                        records.push(rec);
                    }
                    continue;
                } else {
                    next_rec = Some(rec);
                }
            }
            break;
        }

        if keep_records {
            route_ids_used.insert(route_id.clone());
            results.push(Route {
                name: route_id_to_name[&route_id].to_string(),
                stops: records.iter().map(|rec| stop_id_to_pt[&rec[3]]).collect(),
            });
        }

        records.clear();
        if let Some(rec) = next_rec {
            records.push(rec);
        }
    }

    let elapsed = timer.elapsed();
    let dt = elapsed.as_secs() as f64 + f64::from(elapsed.subsec_nanos()) * 1e-9;
    println!("Loading GTFS took {}s", dt);
    Ok(results)
}
