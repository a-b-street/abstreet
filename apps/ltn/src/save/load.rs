use anyhow::Result;
use geojson::{Feature, FeatureCollection, GeoJson, Geometry};
use osm2streets::Direction;
use serde::de::DeserializeOwned;

use geom::{PolyLine, Polygon, Pt2D, Speed};
use map_model::CrossingType;

use crate::{App, FilterType};

// to apply, have bunch of warnings too
// transform it into a set of Commands.. or matching what we do here, just modify the state

pub fn load(app: &mut App, gj: GeoJson) -> Result<()> {
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
            }
            "road filter" => {
                let pt = Pt2D::from_geojson(feature, gps_bounds)?;
                let filter_type: FilterType = serde_prop(feature, "filter_type")?;
            }
            "diagonal filter" => {
                let pl = PolyLine::from_geojson(feature, gps_bounds)?;
                let filter_type: FilterType = serde_prop(feature, "filter_type")?;
            }
            "crossing" => {
                let pt = Pt2D::from_geojson(feature, gps_bounds)?;
                let crossing_kind: CrossingType = serde_prop(feature, "crossing_type")?;
            }
            "one-way" => {
                let pl = PolyLine::from_geojson(feature, gps_bounds)?;
                let direction: Option<Direction> = serde_prop(feature, "direction")?;
            }
            "speed limit" => {
                let pl = PolyLine::from_geojson(feature, gps_bounds)?;
                let speed = Speed::meters_per_second(f64_prop(feature, "speed")?);
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
