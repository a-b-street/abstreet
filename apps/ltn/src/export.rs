use anyhow::Result;
use geo::MapCoordsInPlace;
use geojson::{Feature, FeatureCollection, GeoJson, Value};

use geom::{PolyLine, Pt2D};
use osm2streets::Direction;

use crate::{render, App, Neighbourhood};

pub fn geojson_string(app: &App) -> Result<String> {
    let map = &app.per_map.map;
    let gps_bounds = Some(map.get_gps_bounds());
    let mut features = Vec::new();

    // All neighbourhood boundaries
    for (id, info) in app.partitioning().all_neighbourhoods() {
        let mut feature = Feature::from(info.block.polygon.to_geojson(gps_bounds));
        feature.set_property("type", "neighbourhood");
        features.push(feature);

        // Cells per neighbourhood
        let render_cells = render::RenderCells::new(map, &Neighbourhood::new(app, *id));
        for (idx, mut multipolygon) in render_cells.to_multipolygons().into_iter().enumerate() {
            // Transform to WGS84
            multipolygon.map_coords_in_place(|c| {
                let gps = Pt2D::new(c.x, c.y).to_gps(map.get_gps_bounds());
                (gps.x(), gps.y()).into()
            });
            let mut feature = Feature::from(Value::from(&multipolygon));
            feature.set_property("type", "cell");
            feature.set_property("fill", render_cells.colors[idx].as_hex());
            features.push(feature);
        }
    }

    // All modal filters
    for (road, filter) in map.all_roads_with_modal_filter() {
        if let Ok((pt, angle)) = road.center_pts.dist_along(filter.dist) {
            let road_width = road.get_width();
            let pl = PolyLine::must_new(vec![
                pt.project_away(0.8 * road_width, angle.rotate_degs(90.0)),
                pt.project_away(0.8 * road_width, angle.rotate_degs(-90.0)),
            ]);
            let mut feature = Feature::from(pl.to_geojson(gps_bounds));
            feature.set_property("type", "road filter");
            feature.set_property("filter_type", format!("{:?}", filter.filter_type));
            feature.set_property("stroke", "red");
            features.push(feature);
        }
    }
    for i in map.all_intersections() {
        if let Some(ref filter) = i.modal_filter {
            let pl = filter.geometry(map).to_polyline();
            let mut feature = Feature::from(pl.to_geojson(gps_bounds));
            feature.set_property("type", "diagonal filter");
            feature.set_property("filter_type", format!("{:?}", filter.filter_type));
            feature.set_property("stroke", "red");
            features.push(feature);
        }
    }

    // This includes the direction of every driveable road, not just one-ways. Not sure who's using
    // this export or how, so doesn't matter much.
    for road in map.all_roads() {
        if crate::is_driveable(road, map) {
            let mut feature = Feature::from(road.center_pts.to_geojson(gps_bounds));
            feature.set_property("type", "direction");
            feature.set_property(
                "direction",
                match road.oneway_for_driving() {
                    Some(Direction::Fwd) => "one-way forwards",
                    Some(Direction::Back) => "one-way backwards",
                    None => "two-ways",
                },
            );
            feature.set_property("stroke", "blue");
            features.push(feature);
        }
    }

    for road in map.all_roads() {
        for crossing in &road.crossings {
            let mut feature = Feature::from(
                road.center_pts
                    .must_dist_along(crossing.dist)
                    .0
                    .to_geojson(gps_bounds),
            );
            feature.set_property("type", "crossing");
            feature.set_property("crossing_type", format!("{:?}", crossing.kind));
            features.push(feature);
        }
    }

    let gj = GeoJson::FeatureCollection(FeatureCollection {
        features,
        bbox: None,
        foreign_members: None,
    });

    let x = serde_json::to_string_pretty(&gj)?;
    Ok(x)
}
