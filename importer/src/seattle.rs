use std::collections::HashSet;

use aabb_quadtree::QuadTree;

use abstio::{CityName, MapName};
use abstutil::Timer;
use geom::{Polygon, Ring};
use kml::ExtraShapes;
use map_model::{BuildingID, BuildingType, Map};
use sim::count_parked_cars_per_bldg;
use synthpop::Scenario;

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
    for (b, count) in count_parked_cars_per_bldg(scenario).consume() {
        map.hack_override_offstreet_spots_individ(b, count);
    }
    map.save();
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
