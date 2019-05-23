use geom::{Duration, GPSBounds, LonLat};
use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};

#[derive(Serialize, Deserialize)]
pub struct Trip {
    pub from: LonLat,
    pub to: LonLat,
    // Relative to midnight
    pub depart_at: Duration,
    pub mode: TripMode,

    // TODO Encode way more compactly as (enum, enum)
    pub purpose: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum TripMode {
    Walk,
    Bike,
    Drive,
}

pub fn import_trips(
    path: &str,
    parcels: HashMap<String, LonLat>,
) -> Result<Vec<Trip>, failure::Error> {
    let mut trips = Vec::new();
    for rec in csv::Reader::from_reader(BufReader::new(File::open(path)?)).records() {
        let rec = rec?;

        // opcl
        let from = if let Some(pt) = parcels.get(rec[15].trim_end_matches(".0")) {
            *pt
        } else {
            continue;
        };
        // dpcl
        let to = if let Some(pt) = parcels.get(rec[6].trim_end_matches(".0")) {
            *pt
        } else {
            continue;
        };

        // deptm
        let mins: usize = rec[4].trim_end_matches(".0").parse()?;
        let depart_at = Duration::minutes(mins);

        // mode
        let mode = if let Some(m) = get_mode(&rec[13]) {
            m
        } else {
            continue;
        };

        // opurp and dpurp
        let purpose = format!("{} -> {}", get_purpose(&rec[16]), get_purpose(&rec[7]));

        trips.push(Trip {
            from,
            to,
            depart_at,
            purpose,
            mode,
        });
        // TODO Read all trips
        if trips.len() == 1_000 {
            break;
        }
    }
    Ok(trips)
}

// TODO Do we also need the zone ID, or is parcel ID globally unique?
pub fn import_parcels(path: &str) -> Result<HashMap<String, LonLat>, failure::Error> {
    let mut coords = BufWriter::new(File::create("/tmp/parcels")?);
    let mut parcel_ids = Vec::new();
    // TODO Timer
    for rec in csv::ReaderBuilder::new()
        .delimiter(b' ')
        .from_reader(File::open(path)?)
        .records()
    {
        let rec = rec?;
        parcel_ids.push(rec[15].to_string());
        coords.write_fmt(format_args!("{} {}\n", &rec[25], &rec[26]))?;
        // TODO convert it all
        if parcel_ids.len() == 1_000_000 {
            break;
        }
    }
    coords.flush()?;

    // TODO Ideally we could just do the conversion directly without any dependencies, but the
    // formats are documented quite confusingly. Couldn't get the Rust crate for proj or GDAL
    // bindings to build. So just do this hack.
    let output = std::process::Command::new("cs2cs")
        // cs2cs +init=esri:102748 +to +init=epsg:4326 -f '%.5f' foo
        .args(vec![
            "+init=esri:102748",
            "+to",
            "+init=epsg:4326",
            "-f",
            "%.5f",
            "/tmp/parcels",
        ])
        .output()?;
    assert!(output.status.success());

    let bounds = GPSBounds::seattle_bounds();
    let reader = BufReader::new(output.stdout.as_slice());
    let mut result = HashMap::new();
    for (line, id) in reader.lines().zip(parcel_ids.into_iter()) {
        let line = line?;
        let pieces: Vec<&str> = line.split_whitespace().collect();
        let lon: f64 = pieces[0].parse()?;
        let lat: f64 = pieces[1].parse()?;
        let pt = LonLat::new(lon, lat);
        if bounds.contains(pt) {
            result.insert(id, pt);
        }
    }
    Ok(result)
}

// From https://github.com/psrc/soundcast/wiki/Outputs#trip-file-_triptsv, opurp and dpurp
fn get_purpose(code: &str) -> &str {
    match code {
        "0.0" => "home",
        "1.0" => "work",
        "2.0" => "school",
        "3.0" => "escort",
        "4.0" => "personal business",
        "5.0" => "shopping",
        "6.0" => "meal",
        "7.0" => "social",
        "8.0" => "recreation",
        "9.0" => "medical",
        "10.0" => "park-and-ride transfer",
        _ => panic!("Unknown opurp/dpurp {}", code),
    }
}

// From https://github.com/psrc/soundcast/wiki/Outputs#trip-file-_triptsv, mode
fn get_mode(code: &str) -> Option<TripMode> {
    // TODO I'm not sure how to interpret some of these.
    match code {
        "1.0" | "6.0" => Some(TripMode::Walk),
        "2.0" => Some(TripMode::Bike),
        "3.0" | "4.0" | "5.0" => Some(TripMode::Drive),
        _ => None,
    }
}
