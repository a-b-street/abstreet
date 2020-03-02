use abstutil::{prettyprint_usize, FileWithProgress, Timer};
use geom::{Distance, Duration, FindClosest, LonLat, Pt2D, Time};
use map_model::Map;
use serde_derive::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};

#[derive(Serialize, Deserialize)]
pub struct Trip {
    pub from: Endpoint,
    pub to: Endpoint,
    pub depart_at: Time,
    pub mode: Mode,

    // (household, person within household)
    pub person: (usize, usize),
    // (tour, false is to destination and true is back from dst, trip within half-tour)
    pub seq: (usize, bool, usize),
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
    pub offstreet_parking_spaces: usize,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
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
    let (parcels, metadata, oob_parcels) = import_parcels(parcels_path, timer)?;

    let mut trips = Vec::new();
    let (reader, done) = FileWithProgress::new(trips_path)?;
    let mut total_records = 0;

    for rec in csv::Reader::from_reader(reader).deserialize() {
        total_records += 1;
        let rec: RawTrip = rec?;

        let from = if let Some(f) = parcels.get(&(rec.opcl as usize)) {
            f.clone()
        } else {
            if false {
                println!(
                    "skipping missing from {}",
                    oob_parcels[&(rec.opcl as usize)]
                );
            }
            continue;
        };
        let to = if let Some(t) = parcels.get(&(rec.dpcl as usize)) {
            t.clone()
        } else {
            continue;
        };

        if from.osm_building == to.osm_building {
            // TODO Plumb along pass-through trips later
            if from.osm_building.is_some() {
                /*timer.warn(format!(
                    "Skipping trip from parcel {} to {}; both match OSM building {:?}",
                    rec.opcl, rec.dpcl, from.osm_building
                ));*/
            }
            continue;
        }

        let depart_at = Time::START_OF_DAY + Duration::minutes(rec.deptm as usize);

        let mode = if let Some(m) = get_mode(&rec.mode) {
            m
        } else {
            continue;
        };

        let purpose = (get_purpose(&rec.opurp), get_purpose(&rec.dpurp));

        let trip_time = Duration::f64_minutes(rec.travtime);
        let trip_dist = Distance::miles(rec.travdist);

        let person = (rec.hhno as usize, rec.pno as usize);
        let seq = (rec.tour as usize, rec.half == 2.0, rec.tseg as usize);

        trips.push(Trip {
            from,
            to,
            depart_at,
            purpose,
            mode,
            trip_time,
            trip_dist,
            person,
            seq,
        });
    }
    done(timer);

    timer.note(format!(
        "{} trips total. {} records filtered out",
        prettyprint_usize(trips.len()),
        prettyprint_usize(total_records - trips.len())
    ));

    trips.sort_by_key(|t| t.depart_at);

    Ok((trips, metadata))
}

// TODO Do we also need the zone ID, or is parcel ID globally unique?
// Returns (parcel ID -> Endpoint), (OSM building ID -> metadata), and (parcel ID -> LonLat) of
// filtered parcels
fn import_parcels(
    path: &str,
    timer: &mut Timer,
) -> Result<
    (
        HashMap<usize, Endpoint>,
        BTreeMap<i64, Parcel>,
        HashMap<usize, LonLat>,
    ),
    failure::Error,
> {
    let map = Map::new(abstutil::path_map("huge_seattle"), false, timer);

    // TODO I really just want to do polygon containment with a quadtree. FindClosest only does
    // line-string stuff right now, which'll be weird for the last->first pt line and stuff.
    let mut closest_bldg: FindClosest<i64> = FindClosest::new(map.get_bounds());
    for b in map.all_buildings() {
        closest_bldg.add(b.osm_way_id, b.polygon.points());
    }

    let mut coords = BufWriter::new(File::create("/tmp/parcels")?);
    // (parcel ID, number of households, number of employees, number of parking spots)
    let mut parcel_metadata = Vec::new();

    let (reader, done) = FileWithProgress::new(path)?;
    for rec in csv::ReaderBuilder::new()
        .delimiter(b' ')
        .from_reader(reader)
        .deserialize()
    {
        let rec: RawParcel = rec?;
        // Note parkdy_p and parkhr_p might overlap, so this could be double-counting. >_<
        parcel_metadata.push((
            rec.parcelid,
            rec.hh_p,
            rec.emptot_p,
            rec.parkdy_p + rec.parkhr_p,
        ));
        coords.write_fmt(format_args!("{} {}\n", rec.xcoord_p, rec.ycoord_p))?;
    }
    done(timer);
    coords.flush()?;

    // TODO Ideally we could just do the conversion directly without any dependencies, but the
    // formats are documented quite confusingly. Couldn't get the Rust crate for proj or GDAL
    // bindings to build. So just do this hack.
    timer.start(format!(
        "run cs2cs on {} points",
        prettyprint_usize(parcel_metadata.len())
    ));
    // If you have an ancient version of cs2cs (like from Ubuntu's proj-bin package), the command
    // should instead be:
    // cs2cs +init=esri:102748 +to +init=epsg:4326 -f '%.5f' foo
    let output = std::process::Command::new("cs2cs")
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
    timer.stop(format!(
        "run cs2cs on {} points",
        prettyprint_usize(parcel_metadata.len())
    ));

    let bounds = map.get_gps_bounds();
    let reader = BufReader::new(output.stdout.as_slice());
    let mut result = HashMap::new();
    let mut metadata = BTreeMap::new();
    let mut oob = HashMap::new();
    let orig_parcels = parcel_metadata.len();
    timer.start_iter("read cs2cs output", parcel_metadata.len());
    for (line, (id, num_households, num_employees, offstreet_parking_spaces)) in
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
                .closest_pt(Pt2D::forcibly_from_gps(pt, bounds), Distance::meters(30.0))
                .map(|(b, _)| b);
            if let Some(b) = osm_building {
                metadata.insert(
                    b,
                    Parcel {
                        num_households,
                        num_employees,
                        offstreet_parking_spaces,
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
        } else {
            oob.insert(id, pt);
        }
    }
    timer.note(format!(
        "{} parcels. {} filtered out",
        prettyprint_usize(result.len()),
        prettyprint_usize(orig_parcels - result.len())
    ));
    Ok((result, metadata, oob))
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
        "3.0" | "4.0" | "5.0" => Some(Mode::Drive),
        "6.0" => Some(Mode::Transit),
        // TODO Park-and-ride!
        "7.0" => None,
        // TODO School bus!
        "8.0" => None,
        // TODO Invalid code, what's this one mean?
        "0.0" => None,
        _ => panic!("Unknown mode {}", code),
    }
}

// See https://github.com/psrc/soundcast/wiki/Outputs#trip-file-_triptsv
#[derive(Debug, Deserialize)]
struct RawTrip {
    opcl: f64,
    dpcl: f64,
    deptm: f64,
    mode: String,
    opurp: String,
    dpurp: String,
    travtime: f64,
    travdist: f64,
    hhno: f64,
    pno: f64,
    tour: f64,
    half: f64,
    tseg: f64,
}

// See https://github.com/psrc/soundcast/wiki/Outputs#buffered-parcel-file-buffered_parcelsdat
#[derive(Debug, Deserialize)]
struct RawParcel {
    parcelid: usize,
    hh_p: usize,
    emptot_p: usize,
    parkdy_p: usize,
    parkhr_p: usize,
    xcoord_p: f64,
    ycoord_p: f64,
}
