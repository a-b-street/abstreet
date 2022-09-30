use anyhow::Result;

use geom::{PolyLine, Pt2D};

use crate::{App, Neighbourhood};

/// Returns the path where the file was written
pub fn write_geojson_file(app: &App) -> Result<String> {
    let contents = geojson_string(app)?;
    let path = format!("ltn_{}.geojson", app.per_map.map.get_name().map);
    abstio::write_file(path, contents)
}

fn geojson_string(app: &App) -> Result<String> {
    use geo::MapCoordsInPlace;
    use geojson::{Feature, FeatureCollection, GeoJson, Geometry, Value};

    let map = &app.per_map.map;
    let mut features = Vec::new();

    // All neighbourhood boundaries
    for (id, info) in app.per_map.partitioning.all_neighbourhoods() {
        let mut feature = Feature {
            bbox: None,
            geometry: Some(info.block.polygon.to_geojson(None)),
            id: None,
            properties: None,
            foreign_members: None,
        };
        feature.set_property("type", "neighbourhood");
        features.push(feature);

        // Cells per neighbourhood
        let render_cells = crate::draw_cells::RenderCells::new(map, &Neighbourhood::new(app, *id));
        for (idx, multipolygon) in render_cells.to_multipolygons().into_iter().enumerate() {
            let mut feature = Feature {
                bbox: None,
                geometry: Some(Geometry {
                    bbox: None,
                    value: Value::from(&multipolygon),
                    foreign_members: None,
                }),
                id: None,
                properties: None,
                foreign_members: None,
            };
            feature.set_property("type", "cell");
            feature.set_property("fill", render_cells.colors[idx].as_hex());
            features.push(feature);
        }
    }

    // All modal filters
    for (r, filter) in &app.per_map.edits.roads {
        let road = map.get_r(*r);
        if let Ok((pt, angle)) = road.center_pts.dist_along(filter.dist) {
            let road_width = road.get_width();
            let pl = PolyLine::must_new(vec![
                pt.project_away(0.8 * road_width, angle.rotate_degs(90.0)),
                pt.project_away(0.8 * road_width, angle.rotate_degs(-90.0)),
            ]);
            let mut feature = Feature {
                bbox: None,
                geometry: Some(pl.to_geojson(None)),
                id: None,
                properties: None,
                foreign_members: None,
            };
            feature.set_property("type", "road filter");
            feature.set_property("filter_type", format!("{:?}", filter.filter_type));
            feature.set_property("user_modified", filter.user_modified);
            feature.set_property("stroke", "red");
            features.push(feature);
        }
    }
    for (_, filter) in &app.per_map.edits.intersections {
        let pl = filter.geometry(map).to_polyline();
        let mut feature = Feature {
            bbox: None,
            geometry: Some(pl.to_geojson(None)),
            id: None,
            properties: None,
            foreign_members: None,
        };
        feature.set_property("type", "diagonal filter");
        feature.set_property("filter_type", format!("{:?}", filter.filter_type));
        feature.set_property("stroke", "red");
        features.push(feature);
    }

    // Transform to WGS84
    let gps_bounds = map.get_gps_bounds();
    for feature in &mut features {
        // geojson to geo
        // This could be a Polygon, MultiPolygon, LineString
        let mut geom: geo::Geometry = feature.geometry.take().unwrap().value.try_into()?;

        geom.map_coords_in_place(|c| {
            let gps = Pt2D::new(c.x, c.y).to_gps(gps_bounds);
            (gps.x(), gps.y()).into()
        });

        // geo to geojson
        feature.geometry = Some(Geometry {
            bbox: None,
            value: Value::from(&geom),
            foreign_members: None,
        });
    }

    let gj = GeoJson::FeatureCollection(FeatureCollection {
        features,
        bbox: None,
        foreign_members: None,
    });

    let x = serde_json::to_string_pretty(&gj)?;
    Ok(x)
}
