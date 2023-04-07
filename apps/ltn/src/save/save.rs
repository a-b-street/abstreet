use anyhow::Result;
use geojson::{Feature, FeatureCollection, GeoJson, Geometry};
use osm2streets::Direction;
use serde::de::DeserializeOwned;

use geom::{PolyLine, Polygon, Pt2D, Speed};
use map_model::CrossingType;

use crate::{App, FilterType};

// TODO
// - document it, with the intention that other people can produce it

// TODO tmp just for testing
pub fn write_geojson_file(app: &mut App) -> Result<String> {
    let contents = serde_json::to_string_pretty(&to_new_savefile(app))?;
    let path = format!("ltn_{}.geojson", app.per_map.map.get_name().map);
    let path = abstio::write_file(path, contents)?;

    let gj: geojson::GeoJson = std::fs::read_to_string(&path)?.parse()?;
    super::load::load(app, gj)?;

    Ok(path)
}

pub fn to_new_savefile(app: &App) -> GeoJson {
    let map = &app.per_map.map;
    let gps_bounds = Some(map.get_gps_bounds());
    let mut features = Vec::new();

    // Polygons: neighbourhood boundaries
    // TODO Only modified ones?
    for info in app.partitioning().all_neighbourhoods().values() {
        let mut feature = new_feature(info.block.polygon.to_geojson(gps_bounds));
        feature.set_property("type", "neighbourhood");
        features.push(feature);
    }

    // Points: modal filters on roads
    // TODO Only modified ones, but also remember modifications or deletions of existing
    for (r, filter) in &app.edits().roads {
        let pt = map.get_r(*r).center_pts.must_dist_along(filter.dist).0;
        let mut feature = new_feature(pt.to_geojson(gps_bounds));
        feature.set_property("type", "road filter");
        // TODO or serde into value?
        feature.set_property("filter_type", format!("{:?}", filter.filter_type));
        features.push(feature);
    }

    // LineStrings: diagonal modal filters
    for (_, filter) in &app.edits().intersections {
        let mut feature = new_feature(filter.geometry(map).to_polyline().to_geojson(gps_bounds));
        feature.set_property("type", "diagonal filter");
        feature.set_property("filter_type", format!("{:?}", filter.filter_type));
        features.push(feature);
    }

    // Points: crossings
    // TODO Only modified ones, but also remember modifications/deletions of existing
    for (r, list) in &app.edits().crossings {
        let road = app.per_map.map.get_r(*r);
        for crossing in list {
            let pt = road.center_pts.must_dist_along(crossing.dist).0;
            let mut feature = new_feature(pt.to_geojson(gps_bounds));
            feature.set_property("type", "crossing");
            feature.set_property("crossing_type", format!("{:?}", crossing.kind));
            features.push(feature);
        }
    }

    // LineStrings: one-way changes
    for r in app.edits().one_ways.keys() {
        let road = app.per_map.map.get_r(*r);
        let mut feature = new_feature(road.center_pts.to_geojson(gps_bounds));
        feature.set_property("type", "one-way");
        // TODO serde thing here especially, for None
        feature.set_property("direction", format!("{:?}", road.oneway_for_driving()));
        features.push(feature);
    }

    // LineStrings: speed limit changes
    for (r, speed) in &app.edits().speed_limits {
        let road = app.per_map.map.get_r(*r);
        let mut feature = new_feature(road.center_pts.to_geojson(gps_bounds));
        feature.set_property("type", "speed limit");
        // TODO Again... serde? or better to be explicit about units?
        feature.set_property("speed", speed.inner_meters_per_second());
        features.push(feature);
    }

    // TODO metadata: abst version, map name, proposal name
    let gj = GeoJson::FeatureCollection(FeatureCollection {
        features,
        bbox: None,
        foreign_members: None,
    });
    gj
}

// TODO Helpers to work with geojson crate. Any methods I'm missing that'd help?
fn new_feature(geometry: Geometry) -> Feature {
    Feature {
        bbox: None,
        geometry: Some(geometry),
        id: None,
        properties: None,
        foreign_members: None,
    }
}
