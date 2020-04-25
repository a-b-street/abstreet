use abstutil::{prettyprint_usize, FileWithProgress, Timer};
use geom::{Distance, Duration, FindClosest, LonLat, Pt2D, Time};
use map_model::Map;
use serde_derive::{Deserialize, Serialize};
use sim::TripMode;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::File;
use std::io::Write;

#[derive(Serialize, Deserialize)]
pub struct PopDat {
    pub trips: Vec<OrigTrip>,
    pub parcels: BTreeMap<i64, Parcel>,
}

// Extract trip demand data from PSRC's Soundcast outputs.
pub fn import_data() {
    let mut timer = abstutil::Timer::new("creating popdat");
    let (trips, parcels) = import_trips(
        "../data/input/parcels_urbansim.txt",
        "../data/input/trips_2014.csv",
        &mut timer,
    )
    .unwrap();
    let popdat = PopDat { trips, parcels };
    abstutil::write_binary(abstutil::path_popdat(), &popdat);
}

fn import_trips(
    parcels_path: &str,
    trips_path: &str,
    timer: &mut Timer,
) -> Result<(Vec<OrigTrip>, BTreeMap<i64, Parcel>), failure::Error> {
    let (parcels, metadata) = import_parcels(parcels_path, timer)?;

    if false {
        timer.start("recording parcel IDs");
        let mut f = File::create("parcels.csv")?;
        writeln!(f, "parcel_id")?;
        for id in parcels.keys() {
            writeln!(f, "{}", id)?;
        }
        timer.stop("recording parcel IDs");
    }

    let mut trips = Vec::new();
    let (reader, done) = FileWithProgress::new(trips_path)?;
    let mut total_records = 0;
    let mut people: HashSet<(usize, usize)> = HashSet::new();

    for rec in csv::Reader::from_reader(reader).deserialize() {
        total_records += 1;
        let rec: RawTrip = rec?;

        let from = parcels[&(rec.opcl as usize)].clone();
        let to = parcels[&(rec.dpcl as usize)].clone();

        // If both are None, then skip -- the trip doesn't start or end within huge_seattle.
        // If both are the same building, also skip -- that's a redundant trip.
        if from.osm_building == to.osm_building {
            if from.osm_building.is_some() {
                /*timer.warn(format!(
                    "Skipping trip from parcel {} to {}; both match OSM building {:?}",
                    rec.opcl, rec.dpcl, from.osm_building
                ));*/
            }
            continue;
        }

        let depart_at = Time::START_OF_DAY + Duration::minutes(rec.deptm as usize);

        let mode = get_mode(&rec.mode);
        let purpose = (get_purpose(&rec.opurp), get_purpose(&rec.dpurp));

        let trip_time = Duration::f64_minutes(rec.travtime);
        let trip_dist = Distance::miles(rec.travdist);

        let person = (rec.hhno as usize, rec.pno as usize);
        people.insert(person);
        let seq = (rec.tour as usize, rec.half == 2.0, rec.tseg as usize);

        trips.push(OrigTrip {
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
        "{} trips total, over {} people. {} records filtered out",
        prettyprint_usize(trips.len()),
        prettyprint_usize(people.len()),
        prettyprint_usize(total_records - trips.len())
    ));

    trips.sort_by_key(|t| t.depart_at);

    Ok((trips, metadata))
}

// TODO Do we also need the zone ID, or is parcel ID globally unique?
// Returns (parcel ID -> Endpoint) and (OSM building ID -> metadata)
fn import_parcels(
    path: &str,
    timer: &mut Timer,
) -> Result<(HashMap<usize, Endpoint>, BTreeMap<i64, Parcel>), failure::Error> {
    let map = Map::new(abstutil::path_map("huge_seattle"), false, timer);

    // TODO I really just want to do polygon containment with a quadtree. FindClosest only does
    // line-string stuff right now, which'll be weird for the last->first pt line and stuff.
    let mut closest_bldg: FindClosest<i64> = FindClosest::new(map.get_bounds());
    for b in map.all_buildings() {
        closest_bldg.add(b.osm_way_id, b.polygon.points());
    }

    let mut x_coords: Vec<f64> = Vec::new();
    let mut y_coords: Vec<f64> = Vec::new();
    // Dummy values
    let mut z_coords: Vec<f64> = Vec::new();
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
        x_coords.push(rec.xcoord_p);
        y_coords.push(rec.ycoord_p);
        z_coords.push(0.0);
    }
    done(timer);

    timer.start(format!("transform {} points", parcel_metadata.len()));

    // From https://epsg.io/102748 to https://epsg.io/4326
    let transform = gdal::spatial_ref::CoordTransform::new(
        &gdal::spatial_ref::SpatialRef::from_proj4(
            "+proj=lcc +lat_1=47.5 +lat_2=48.73333333333333 +lat_0=47 +lon_0=-120.8333333333333 \
             +x_0=500000.0000000002 +y_0=0 +datum=NAD83 +units=us-ft +no_defs",
        )
        .expect("washington state plane"),
        &gdal::spatial_ref::SpatialRef::from_epsg(4326).unwrap(),
    )
    .expect("regular GPS");
    transform
        .transform_coords(&mut x_coords, &mut y_coords, &mut z_coords)
        .expect("transform coords");

    timer.stop(format!("transform {} points", parcel_metadata.len()));

    let bounds = map.get_gps_bounds();
    let mut result = HashMap::new();
    let mut metadata = BTreeMap::new();
    timer.start_iter("finalize parcel output", parcel_metadata.len());
    for ((x, y), (id, num_households, num_employees, offstreet_parking_spaces)) in x_coords
        .into_iter()
        .zip(y_coords.into_iter())
        .zip(parcel_metadata.into_iter())
    {
        timer.next();
        let pt = LonLat::new(x, y);
        let osm_building = if bounds.contains(pt) {
            closest_bldg
                .closest_pt(Pt2D::forcibly_from_gps(pt, bounds), Distance::meters(30.0))
                .map(|(b, _)| b)
        } else {
            None
        };
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
                parcel_id: id,
            },
        );
    }
    timer.note(format!("{} parcels", prettyprint_usize(result.len())));
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
fn get_mode(code: &str) -> TripMode {
    match code {
        "1.0" => TripMode::Walk,
        "2.0" => TripMode::Bike,
        "3.0" | "4.0" | "5.0" => TripMode::Drive,
        // TODO Park-and-ride and school bus as walk-to-transit is a little weird.
        "6.0" | "7.0" | "8.0" => TripMode::Transit,
        // TODO Invalid code, what's this one mean? I only see a few examples, so just default to
        // walking.
        "0.0" => TripMode::Walk,
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OrigTrip {
    pub from: Endpoint,
    pub to: Endpoint,
    pub depart_at: Time,
    pub mode: TripMode,

    // (household, person within household)
    pub person: (usize, usize),
    // (tour, false is to destination and true is back from dst, trip within half-tour)
    pub seq: (usize, bool, usize),
    pub purpose: (Purpose, Purpose),
    pub trip_time: Duration,
    pub trip_dist: Distance,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Endpoint {
    pub pos: LonLat,
    pub osm_building: Option<i64>,
    pub parcel_id: usize,
}

#[derive(Serialize, Deserialize)]
pub struct Parcel {
    pub num_households: usize,
    pub num_employees: usize,
    pub offstreet_parking_spaces: usize,
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
