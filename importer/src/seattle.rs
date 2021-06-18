use std::collections::{BTreeMap, HashSet};
use std::fs::File;

use aabb_quadtree::QuadTree;
use serde::Deserialize;

use abstio::{CityName, MapName};
use abstutil::{MultiMap, Timer};
use geom::{Distance, Duration, Polygon, Ring, Time};
use kml::ExtraShapes;
use map_model::{BuildingID, BuildingType, BusRouteID, Map};
use sim::Scenario;

use crate::configuration::ImporterConfiguration;
use crate::utils::{download, download_kml, osmconvert};

async fn input(config: &ImporterConfiguration, timer: &mut Timer<'_>) {
    let city = CityName::seattle();

    download(
        config,
        city.input_path("osm/washington-latest.osm.pbf"),
        "http://download.geofabrik.de/north-america/us/washington-latest.osm.pbf",
    )
    .await;
    // Soundcast data comes from https://github.com/psrc/soundcast/releases
    download(
        config,
        city.input_path("parcels_urbansim.txt"),
        "https://www.dropbox.com/s/t9oug9lwhdwfc04/psrc_2014.zip?dl=0",
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

pub async fn osm_to_raw(name: &str, timer: &mut Timer<'_>, config: &ImporterConfiguration) {
    let city = CityName::seattle();

    input(config, timer).await;
    osmconvert(
        city.input_path("osm/washington-latest.osm.pbf"),
        format!("importer/config/us/seattle/{}.poly", name),
        city.input_path(format!("osm/{}.osm", name)),
        config,
    );

    let map = convert_osm::convert(
        convert_osm::Options {
            osm_input: city.input_path(format!("osm/{}.osm", name)),
            name: MapName::seattle(name),

            clip: Some(format!("importer/config/us/seattle/{}.poly", name)),
            map_config: map_model::MapConfig {
                driving_side: map_model::DrivingSide::Right,
                bikes_can_use_bus_lanes: true,
                inferred_sidewalks: true,
                street_parking_spot_length: Distance::meters(8.0),
            },

            onstreet_parking: convert_osm::OnstreetParking::Blockface(
                city.input_path("blockface.bin"),
            ),
            public_offstreet_parking: convert_osm::PublicOffstreetParking::Gis(
                city.input_path("offstreet_parking.bin"),
            ),
            private_offstreet_parking: convert_osm::PrivateOffstreetParking::FixedPerBldg(
                // TODO Utter guesses or in response to gridlock
                match name {
                    "downtown" => 5,
                    "lakeslice" => 5,
                    "qa" => 5,
                    "rainier_valley" => 3,
                    "south_seattle" => 5,
                    "udistrict" => 5,
                    "wallingford" => 5,
                    _ => 1,
                },
            ),
            // They mess up 16th and E Marginal badly enough to cause gridlock.
            include_railroads: false,
            extra_buildings: None,
            gtfs: Some(city.input_path("google_transit")),
        },
        timer,
    );
    map.save();
}

/// Download and pre-process data needed to generate Seattle scenarios.
pub async fn ensure_popdat_exists(
    timer: &mut Timer<'_>,
    config: &ImporterConfiguration,
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
        osm_to_raw("huge_seattle", timer, config).await;
    }
    let huge_map = if abstio::file_exists(huge_name.path()) {
        map_model::Map::load_synchronously(huge_name.path(), timer)
    } else {
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
