use anyhow::Result;

use geom::{PolyLine, Pt2D};
use widgetry::EventCtx;

use crate::{App, Neighborhood};

/// Returns the path where the file was written
pub fn write_geojson_file(ctx: &EventCtx, app: &App) -> Result<String> {
    let contents = geojson_string(ctx, app)?;
    let path = format!("ltn_{}.geojson", app.map.get_name().map);

    // TODO Refactor into map_gui or abstio and handle errors better
    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::JsCast;

        let data: String = js_sys::JsString::from("data:text/json;charset=utf-8,")
            .concat(&js_sys::encode_uri_component(&contents))
            .into();

        let window = web_sys::window().unwrap();
        let document = window.document().unwrap();
        let node = document
            .create_element("a")
            .unwrap()
            .dyn_into::<web_sys::HtmlElement>()
            .unwrap();
        node.set_attribute("href", &data).unwrap();
        node.set_attribute("download", &path).unwrap();
        document.body().unwrap().append_child(&node).unwrap();
        node.click();
        node.remove();
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::io::Write;

        let mut file = fs_err::File::create(&path)?;
        write!(file, "{}", contents)?;
    }

    Ok(path)
}

fn geojson_string(ctx: &EventCtx, app: &App) -> Result<String> {
    use geo::algorithm::map_coords::MapCoordsInplace;
    use geojson::{Feature, FeatureCollection, GeoJson, Geometry, Value};

    let map = &app.map;
    let mut features = Vec::new();

    // All neighborhood boundaries
    for (id, (block, color)) in app.session.partitioning.all_neighborhoods() {
        let mut feature = Feature {
            bbox: None,
            geometry: Some(block.polygon.to_geojson(None)),
            id: None,
            properties: None,
            foreign_members: None,
        };
        feature.set_property("type", "neighborhood");
        feature.set_property("fill", color.as_hex());
        // Cells should cover these up
        feature.set_property("fill-opacity", 0.0);
        features.push(feature);

        // Cells per neighborhood
        let render_cells =
            crate::draw_cells::RenderCells::new(map, &Neighborhood::new(ctx, app, *id));
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
    for (r, dist) in &app.session.modal_filters.roads {
        let road = map.get_r(*r);
        if let Ok((pt, angle)) = road.center_pts.dist_along(*dist) {
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
            feature.set_property("stroke", "red");
            features.push(feature);
        }
    }
    for (_, filter) in &app.session.modal_filters.intersections {
        let pl = filter.geometry(map).to_polyline();
        let mut feature = Feature {
            bbox: None,
            geometry: Some(pl.to_geojson(None)),
            id: None,
            properties: None,
            foreign_members: None,
        };
        feature.set_property("type", "diagonal filter");
        feature.set_property("stroke", "red");
        features.push(feature);
    }

    // Transform to WGS84
    let gps_bounds = map.get_gps_bounds();
    for feature in &mut features {
        // geojson to geo
        // This could be a Polygon, MultiPolygon, LineString
        let mut geom: geo::Geometry<f64> = feature.geometry.take().unwrap().value.try_into()?;

        geom.map_coords_inplace(|c| {
            let gps = Pt2D::new(c.0, c.1).to_gps(gps_bounds);
            (gps.x(), gps.y())
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
