use abstutil::{prettyprint_usize, skip_fail, FileWithProgress, Timer};
use geom::{Distance, Duration, FindClosest, LonLat, Pt2D};
use map_model::Map;
use serde_derive::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};

#[derive(Serialize, Deserialize)]
pub struct Trip {
    pub from: Endpoint,
    pub to: Endpoint,
    // Relative to midnight
    pub depart_at: Duration,
    pub mode: Mode,

    pub purpose: (Purpose, Purpose),
    pub trip_time: Duration,
    pub trip_dist: Distance,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Endpoint {
    pub pos: LonLat,
    pub osm_building: Option<i64>,
}

#[derive(Serialize, Deserialize)]
pub struct Parcel {
    pub num_households: usize,
    pub num_employees: usize,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum Mode {
    Walk,
    Bike,
    Drive,
    Transit,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum Purpose {
    Home,
    Work,
    School,
    Escort,
    PersonalBusiness,
    Shopping,
    Meal,
    Social,
    Recreation,
    Medical,
    ParkAndRideTransfer,
}

pub fn import_trips(
    parcels_path: &str,
    trips_path: &str,
    timer: &mut Timer,
) -> Result<(Vec<Trip>, BTreeMap<i64, Parcel>), failure::Error> {
    let (parcels, metadata) = import_parcels(parcels_path, timer)?;

    let mut trips = Vec::new();
    let (reader, done) = FileWithProgress::new(trips_path)?;
    for rec in csv::Reader::from_reader(reader).records() {
        let rec = rec?;

        // opcl
        let from = skip_fail!(parcels.get(rec[15].trim_end_matches(".0"))).clone();
        // dpcl
        let to = skip_fail!(parcels.get(rec[6].trim_end_matches(".0"))).clone();

        if from.osm_building == to.osm_building {
            // TODO Plumb along pass-through trips later
            if from.osm_building.is_some() {
                timer.warn(format!(
                    "Skipping trip from parcel {} to {}; both match OSM building {:?}",
                    &rec[15], &rec[6], from.osm_building
                ));
            }
            continue;
        }

        // deptm
        let depart_at = Duration::minutes(rec[4].trim_end_matches(".0").parse::<usize>()?);

        // mode
        let mode = skip_fail!(get_mode(&rec[13]));

        // opurp and dpurp
        let purpose = (get_purpose(&rec[16]), get_purpose(&rec[7]));

        // travtime
        let trip_time = Duration::f64_minutes(rec[25].parse::<f64>()?);
        // travdist
        let trip_dist = Distance::miles(rec[24].parse::<f64>()?);

        trips.push(Trip {
            from,
            to,
            depart_at,
            purpose,
            mode,
            trip_time,
            trip_dist,
        });
    }
    done(timer);

    timer.note(format!("{} trips total", prettyprint_usize(trips.len())));

    trips.sort_by_key(|t| t.depart_at);

    Ok((trips, metadata))
}

// TODO Do we also need the zone ID, or is parcel ID globally unique?
// Returns (parcel ID -> Endpoint) and (OSM building ID -> metadata)
fn import_parcels(
    path: &str,
    timer: &mut Timer,
) -> Result<(HashMap<String, Endpoint>, BTreeMap<i64, Parcel>), failure::Error> {
    let map: Map = abstutil::read_binary(&abstutil::path_map("huge_seattle"), timer)?;

    // TODO I really just want to do polygon containment with a quadtree. FindClosest only does
    // line-string stuff right now, which'll be weird for the last->first pt line and stuff.
    let mut closest_bldg: FindClosest<i64> = FindClosest::new(map.get_bounds());
    for b in map.all_buildings() {
        closest_bldg.add(b.osm_way_id, b.polygon.points());
    }

    let mut coords = BufWriter::new(File::create("/tmp/parcels")?);
    // (parcel ID, number of households, number of employees)
    let mut parcel_metadata = Vec::new();

    let (reader, done) = FileWithProgress::new(path)?;
    for rec in csv::ReaderBuilder::new()
        .delimiter(b' ')
        .from_reader(reader)
        .records()
    {
        let rec = rec?;
        // parcelid, hh_p, emptot_p
        parcel_metadata.push((
            rec[15].to_string(),
            rec[12].parse::<usize>()?,
            rec[11].parse::<usize>()?,
        ));
        coords.write_fmt(format_args!("{} {}\n", &rec[25], &rec[26]))?;
    }
    done(timer);
    coords.flush()?;

    // TODO Ideally we could just do the conversion directly without any dependencies, but the
    // formats are documented quite confusingly. Couldn't get the Rust crate for proj or GDAL
    // bindings to build. So just do this hack.
    timer.start(&format!(
        "run cs2cs on {} points",
        prettyprint_usize(parcel_metadata.len())
    ));
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
    timer.stop(&format!(
        "run cs2cs on {} points",
        prettyprint_usize(parcel_metadata.len())
    ));

    let bounds = map.get_gps_bounds();
    let reader = BufReader::new(output.stdout.as_slice());
    let mut result = HashMap::new();
    let mut metadata = BTreeMap::new();
    timer.start_iter("read cs2cs output", parcel_metadata.len());
    for (line, (id, num_households, num_employees)) in
        reader.lines().zip(parcel_metadata.into_iter())
    {
        timer.next();
        let line = line?;
        let pieces: Vec<&str> = line.split_whitespace().collect();
        let lon: f64 = pieces[0].parse()?;
        let lat: f64 = pieces[1].parse()?;
        let pt = LonLat::new(lon, lat);
        if bounds.contains(pt) {
            let osm_building = closest_bldg
                .closest_pt(Pt2D::from_gps(pt, bounds).unwrap(), Distance::meters(30.0))
                .map(|(b, _)| b);
            if let Some(b) = osm_building {
                metadata.insert(
                    b,
                    Parcel {
                        num_households,
                        num_employees,
                    },
                );
            }
            result.insert(
                id,
                Endpoint {
                    pos: pt,
                    osm_building,
                },
            );
        }
    }
    Ok((result, metadata))
}

// From https://github.com/psrc/soundcast/wiki/Outputs#trip-file-_triptsv, opurp and dpurp
fn get_purpose(code: &str) -> Purpose {
    match code {
        "0.0" => Purpose::Home,
        "1.0" => Purpose::Work,
        "2.0" => Purpose::School,
        "3.0" => Purpose::Escort,
        "4.0" => Purpose::PersonalBusiness,
        "5.0" => Purpose::Shopping,
        "6.0" => Purpose::Meal,
        "7.0" => Purpose::Social,
        "8.0" => Purpose::Recreation,
        "9.0" => Purpose::Medical,
        "10.0" => Purpose::ParkAndRideTransfer,
        _ => panic!("Unknown opurp/dpurp {}", code),
    }
}

// From https://github.com/psrc/soundcast/wiki/Outputs#trip-file-_triptsv, mode
fn get_mode(code: &str) -> Option<Mode> {
    match code {
        "1.0" => Some(Mode::Walk),
        "2.0" => Some(Mode::Bike),
        "6.0" => Some(Mode::Transit),
        "3.0" | "4.0" | "5.0" => Some(Mode::Drive),
        _ => None,
    }
}
