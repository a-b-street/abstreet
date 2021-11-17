use std::collections::{BTreeMap, HashSet};
use std::fs::File;

use aabb_quadtree::QuadTree;
use serde::Deserialize;

use abstio::{CityName, MapName};
use abstutil::{MultiMap, Timer};
use geom::{Duration, Polygon, Ring, Time};
use kml::ExtraShapes;
use map_model::{BuildingID, BuildingType, BusRouteID, Map};
use sim::Scenario;

use crate::configuration::ImporterConfiguration;
use crate::utils::{download, download_kml};

pub async fn input(config: &ImporterConfiguration, timer: &mut Timer<'_>) {
    let city = CityName::seattle();

    // Soundcast data was originally retrieved from staff at PSRC via a download link that didn't
    // last long. From that original 2014 .zip (possibly still available from
    // https://github.com/psrc/soundcast/releases), two files were extracted --
    // parcels_urbansim.txt and trips_2014.csv. Those are now stored in S3. It's a bit weird for
    // the importer pipeline to depend on something in data/input in S3, but this should let
    // anybody run the full pipeline.
    download(
        config,
        city.input_path("parcels_urbansim.txt"),
        "http://abstreet.s3-website.us-east-2.amazonaws.com/dev/data/input/us/seattle/parcels_urbansim.txt.gz",
    )
    .await;
    download(
        config,
        city.input_path("trips_2014.csv"),
        "http://abstreet.s3-website.us-east-2.amazonaws.com/dev/data/input/us/seattle/trips_2014.csv.gz",
    )
    .await;

    let bounds = geom::GPSBounds::from(
        geom::LonLat::read_osmosis_polygon("importer/config/us/seattle/huge_seattle.poly").unwrap(),
    );
    // From http://data-seattlecitygis.opendata.arcgis.com/datasets/blockface
    download_kml(
        city.input_path("blockface.bin"),
        "https://opendata.arcgis.com/datasets/a1458ad1abca41869b81f7c0db0cd777_0.kml",
        &bounds,
        true,
        timer,
    )
    .await;
    // From https://data-seattlecitygis.opendata.arcgis.com/datasets/public-garages-or-parking-lots
    download_kml(
        city.input_path("offstreet_parking.bin"),
        "http://data-seattlecitygis.opendata.arcgis.com/datasets/8e52dfde6d5d45948f7a90654c8d50cd_0.kml",
        &bounds,
        true,
        timer
    ).await;

    download(
        config,
        city.input_path("google_transit/"),
        "http://metro.kingcounty.gov/gtfs/google_transit.zip",
    )
    .await;

    // From
    // https://data-seattlecitygis.opendata.arcgis.com/datasets/5b5c745e0f1f48e7a53acec63a0022ab_0
    download(
        config,
        city.input_path("collisions.kml"),
        "https://opendata.arcgis.com/datasets/5b5c745e0f1f48e7a53acec63a0022ab_0.kml",
    )
    .await;

    // This is a little expensive, so delete data/input/us/seattle/collisions.bin to regenerate
    // this.
    if !abstio::file_exists(city.input_path("collisions.bin")) {
        let shapes = kml::load(city.input_path("collisions.kml"), &bounds, true, timer).unwrap();
        let collisions = collisions::import_seattle(
            shapes,
            "https://data-seattlecitygis.opendata.arcgis.com/datasets/5b5c745e0f1f48e7a53acec63a0022ab_0");
        abstio::write_binary(city.input_path("collisions.bin"), &collisions);
    }

    // From https://data-seattlecitygis.opendata.arcgis.com/datasets/parcels-1
    download_kml(
        city.input_path("zoning_parcels.bin"),
        "https://opendata.arcgis.com/datasets/42863f1debdc47488a1c2b9edd38053e_2.kml",
        &bounds,
        true,
        timer,
    )
    .await;

    // From
    // https://data-seattlecitygis.opendata.arcgis.com/datasets/current-land-use-zoning-detail
    download_kml(
        city.input_path("land_use.bin"),
        "https://opendata.arcgis.com/datasets/dd29065b5d01420e9686570c2b77502b_0.kml",
        &bounds,
        false,
        timer,
    )
    .await;
}

/// Download and pre-process data needed to generate Seattle scenarios.
pub async fn ensure_popdat_exists(
    timer: &mut Timer<'_>,
    config: &ImporterConfiguration,
    built_raw_huge_seattle: &mut bool,
    built_map_huge_seattle: &mut bool,
) -> (crate::soundcast::PopDat, map_model::Map) {
    let huge_name = MapName::seattle("huge_seattle");

    if abstio::file_exists(abstio::path_popdat()) {
        println!("- {} exists, not regenerating it", abstio::path_popdat());
        return (
            abstio::read_binary(abstio::path_popdat(), timer),
            map_model::Map::load_synchronously(huge_name.path(), timer),
        );
    }

    if !abstio::file_exists(abstio::path_raw_map(&huge_name)) {
        input(config, timer).await;
        crate::utils::osm_to_raw(MapName::seattle("huge_seattle"), timer, config).await;
        *built_raw_huge_seattle = true;
    }
    let huge_map = if abstio::file_exists(huge_name.path()) {
        map_model::Map::load_synchronously(huge_name.path(), timer)
    } else {
        *built_map_huge_seattle = true;
        crate::utils::raw_to_map(&huge_name, map_model::RawToMapOptions::default(), timer)
    };

    (crate::soundcast::import_data(&huge_map, timer), huge_map)
}

pub fn adjust_private_parking(map: &mut Map, scenario: &Scenario) {
    for (b, count) in scenario.count_parked_cars_per_bldg().consume() {
        map.hack_override_offstreet_spots_individ(b, count);
    }
    map.save();
}

/// This import from GTFS:
/// - is specific to Seattle, whose files don't seem to match https://developers.google.com/transit/gtfs/reference
/// - is probably wrong
pub fn add_gtfs_schedules(map: &mut Map) {
    let city = CityName::seattle();
    // https://www.openstreetmap.org/relation/8616968 as an example, mapping to
    // https://kingcounty.gov/depts/transportation/metro/schedules-maps/route/048.aspx

    let mut trip_marker_to_route: BTreeMap<String, BusRouteID> = BTreeMap::new();
    for br in map.all_bus_routes() {
        if let Some(ref m) = br.gtfs_trip_marker {
            // Dunno what the :0 thing is
            trip_marker_to_route.insert(m.split(':').next().unwrap().to_string(), br.id);
        }
    }

    // Each route has a bunch of trips throughout the day
    let mut trip_marker_to_trips: MultiMap<String, String> = MultiMap::new();
    for rec in
        csv::Reader::from_reader(File::open(city.input_path("google_transit/trips.txt")).unwrap())
            .deserialize()
    {
        let rec: TripRecord = rec.unwrap();
        if trip_marker_to_route.contains_key(&rec.shape_id) {
            trip_marker_to_trips.insert(rec.shape_id, rec.trip_id);
        }
    }

    // For every trip, find the earliest arrival time. That should be the spawn time.
    let mut trip_to_earliest_time: BTreeMap<String, Time> = BTreeMap::new();
    for rec in csv::Reader::from_reader(
        File::open(city.input_path("google_transit/stop_times.txt")).unwrap(),
    )
    .deserialize()
    {
        let rec: StopTimeRecord = rec.unwrap();
        let mut time = Time::parse(&rec.arrival_time).unwrap();
        // Maybe we should duplicate these to handle beginning and end of the simulation
        if time > Time::START_OF_DAY + Duration::hours(24) {
            time = time - Duration::hours(24);
        }
        if trip_to_earliest_time
            .get(&rec.trip_id)
            .map(|t| time < *t)
            .unwrap_or(true)
        {
            trip_to_earliest_time.insert(rec.trip_id, time);
        }
    }

    // Collect the spawn times per route
    for (marker, trips) in trip_marker_to_trips.consume() {
        let mut times = Vec::new();
        for trip_id in trips {
            times.push(trip_to_earliest_time.remove(&trip_id).unwrap());
        }
        times.sort();
        times.dedup();

        let br = trip_marker_to_route.remove(&marker).unwrap();
        map.hack_override_orig_spawn_times(br, times);
    }
    map.save();
}

#[derive(Debug, Deserialize)]
struct TripRecord {
    shape_id: String,
    trip_id: String,
}

#[derive(Debug, Deserialize)]
struct StopTimeRecord {
    trip_id: String,
    arrival_time: String,
}

/// Match OSM buildings to parcels, scraping the number of housing units.
// TODO It's expensive to load the huge zoning_parcels.bin file for every map.
pub fn match_parcels_to_buildings(map: &mut Map, shapes: &ExtraShapes, timer: &mut Timer) {
    let mut parcels_with_housing: Vec<(Polygon, usize)> = Vec::new();
    // TODO We should refactor something like FindClosest, but for polygon containment
    // The quadtree's ID is just an index into parcels_with_housing.
    let mut quadtree: QuadTree<usize> = QuadTree::default(map.get_bounds().as_bbox());
    timer.start_iter("index all parcels", shapes.shapes.len());
    for shape in &shapes.shapes {
        timer.next();
        if let Some(units) = shape
            .attributes
            .get("EXIST_UNITS")
            .and_then(|x| x.parse::<usize>().ok())
        {
            if let Some(ring) = map
                .get_gps_bounds()
                .try_convert(&shape.points)
                .and_then(|pts| Ring::new(pts).ok())
            {
                let polygon = ring.into_polygon();
                quadtree
                    .insert_with_box(parcels_with_housing.len(), polygon.get_bounds().as_bbox());
                parcels_with_housing.push((polygon, units));
            }
        }
    }

    let mut used_parcels: HashSet<usize> = HashSet::new();
    let mut units_per_bldg: Vec<(BuildingID, usize)> = Vec::new();
    timer.start_iter("match buildings to parcels", map.all_buildings().len());
    for b in map.all_buildings() {
        timer.next();
        // If multiple parcels contain a building's center, just pick one arbitrarily
        for (idx, _, _) in quadtree.query(b.polygon.get_bounds().as_bbox()) {
            let idx = *idx;
            if used_parcels.contains(&idx)
                || !parcels_with_housing[idx].0.contains_pt(b.label_center)
            {
                continue;
            }
            used_parcels.insert(idx);
            units_per_bldg.push((b.id, parcels_with_housing[idx].1));
        }
    }

    for (b, num_housing_units) in units_per_bldg {
        let bldg_type = match map.get_b(b).bldg_type.clone() {
            BuildingType::Residential { num_residents, .. } => BuildingType::Residential {
                num_housing_units,
                num_residents,
            },
            x => x,
        };
        map.hack_override_bldg_type(b, bldg_type);
    }

    map.save();
}
