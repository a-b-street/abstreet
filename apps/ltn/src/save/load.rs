use anyhow::Result;
use blockfinding::{Block, Perimeter};
use geojson::{Feature, FeatureCollection, GeoJson, Geometry};
use osm2streets::Direction;
use serde::de::DeserializeOwned;

use geom::{Distance, FindClosest, PolyLine, Polygon, Pt2D, Speed};
use map_model::{CrossingType, RoadID};

use crate::logic::BlockID;
use crate::{App, FilterType};

// to apply, have bunch of warnings too
// transform it into a set of Commands.. or matching what we do here, just modify the state

pub fn load(app: &mut App, gj: GeoJson) -> Result<()> {
    let mut road_centers: FindClosest<RoadID> = FindClosest::new();
    for r in app.per_map.map.all_roads() {
        // TODO Could filter for roads valid to have crossings, filters, etc
        road_centers.add_polyline(r.id, &r.center_pts);
    }
    let threshold = Distance::meters(10.0);

    let gps_bounds = Some(app.per_map.map.get_gps_bounds());
    let features: Vec<Feature> = if let GeoJson::FeatureCollection(collection) = gj {
        collection.features
    } else {
        bail!("Input isn't a FeatureCollection");
    };
    for feature in &features {
        match string_prop(feature, "type")? {
            "neighbourhood" => {
                let polygon = Polygon::from_geojson_new(feature, gps_bounds)?;

                let block = recover_large_block(app, polygon)?;
            }
            "road filter" => {
                let pt = Pt2D::from_geojson(feature, gps_bounds)?;
                let filter_type: FilterType = serde_prop(feature, "filter_type")?;

                if let Some((r, _, dist)) = road_centers.closest_pt_on_line(pt, threshold) {
                    info!("found filter at {r}, {dist}");
                }
            }
            "diagonal filter" => {
                let pl = PolyLine::from_geojson(feature, gps_bounds)?;
                let filter_type: FilterType = serde_prop(feature, "filter_type")?;

                // TODO Closest intersection, then figure out the angle
            }
            "crossing" => {
                let pt = Pt2D::from_geojson(feature, gps_bounds)?;
                let crossing_kind: CrossingType = serde_prop(feature, "crossing_type")?;

                if let Some((r, _, dist)) = road_centers.closest_pt_on_line(pt, threshold) {
                    info!("found crossing at {r}, {dist}");
                }
            }
            "one-way" => {
                let pl = PolyLine::from_geojson(feature, gps_bounds)?;
                let direction: Option<Direction> = serde_prop(feature, "direction")?;

                // TODO Map matching
            }
            "speed limit" => {
                let pl = PolyLine::from_geojson(feature, gps_bounds)?;
                let speed = Speed::meters_per_second(f64_prop(feature, "speed")?);

                // TODO Map matching
            }
            x => bail!("Unknown type {x}"),
        }
    }
    Ok(())
}

fn string_prop<'a>(f: &'a Feature, key: &str) -> Result<&'a str> {
    if let Some(value) = f.property(key) {
        if let Some(string) = value.as_str() {
            return Ok(string);
        }
        bail!("{key} isn't a string");
    }
    bail!("feature is missing {key}");
}

fn f64_prop(f: &Feature, key: &str) -> Result<f64> {
    if let Some(value) = f.property(key) {
        if let Some(string) = value.as_f64() {
            return Ok(string);
        }
        bail!("{key} isn't a float");
    }
    bail!("feature is missing {key}");
}

fn serde_prop<T: DeserializeOwned>(f: &Feature, key: &str) -> Result<T> {
    // TODO remove_property instead
    if let Some(value) = f.property(key) {
        serde_json::from_value(value.clone()).map_err(|err| err.into())
    } else {
        bail!("feature is missing {key}");
    }
}

// TODO Move to blockfinding crate
fn recover_large_block(app: &App, raw_polygon: Polygon) -> Result<(Block, Vec<BlockID>)> {
    // Find what this polygon intersects
    // TODO Partly, completely, or more than 50%? Think through likely changes
    // TODO Use a quadtree to prune
    let mut block_ids = Vec::new();
    let mut perimeters = Vec::new();
    for (id, block) in app.partitioning().all_single_blocks() {
        if raw_polygon.intersects(&block.polygon) {
            block_ids.push(id);
            perimeters.push(block.perimeter.clone());
        }
    }
    if block_ids.is_empty() {
        bail!("No single blocks match the input");
    }

    let stepwise_debug = false;
    let mut result = Perimeter::merge_all(&app.per_map.map, perimeters, stepwise_debug);
    if result.len() != 1 {
        bail!(
            "After merging {} blocks, got {} results",
            block_ids.len(),
            result.len()
        );
    }
    let final_block = result.pop().unwrap().to_block(&app.per_map.map)?;
    Ok((final_block, block_ids))
}
