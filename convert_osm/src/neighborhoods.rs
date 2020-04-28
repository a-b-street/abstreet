use abstutil::Timer;
use geojson::{GeoJson, PolygonType, Value};
use geom::{GPSBounds, LonLat};
use map_model::NeighborhoodBuilder;

pub fn convert(geojson_path: String, city_name: String, map_name: String, gps_bounds: &GPSBounds) {
    println!("Extracting neighborhoods from {}...", geojson_path);
    let document: GeoJson = abstutil::read_json(geojson_path, &mut Timer::throwaway());
    match document {
        GeoJson::FeatureCollection(c) => {
            for f in c.features.into_iter() {
                let name = f.properties.unwrap()["name"].as_str().unwrap().to_string();
                match f.geometry.unwrap().value {
                    Value::Polygon(p) => {
                        convert_polygon(p, name, city_name.clone(), map_name.clone(), gps_bounds);
                    }
                    Value::MultiPolygon(polygons) => {
                        for (idx, p) in polygons.into_iter().enumerate() {
                            convert_polygon(
                                p,
                                format!("{} portion #{}", name, idx + 1),
                                city_name.clone(),
                                map_name.clone(),
                                gps_bounds,
                            );
                        }
                    }
                    x => panic!("Unexpected GeoJson value {:?}", x),
                }
            }
        }
        _ => panic!("Unexpected GeoJson root {:?}", document),
    }
}

fn convert_polygon(
    input: PolygonType,
    name: String,
    city_name: String,
    map_name: String,
    gps_bounds: &GPSBounds,
) {
    if input.len() > 1 {
        println!("{} has a polygon with an inner ring, skipping", name);
        return;
    }

    let mut points: Vec<LonLat> = Vec::new();
    for raw_pt in &input[0] {
        assert_eq!(raw_pt.len(), 2);
        let pt = LonLat::new(raw_pt[0], raw_pt[1]);
        if gps_bounds.contains(pt) {
            points.push(pt);
        } else {
            println!(
                "Neighborhood polygon \"{}\" is out-of-bounds, skipping",
                name
            );
            return;
        }
    }
    NeighborhoodBuilder {
        city_name,
        map_name,
        name,
        points,
    }
    .save();
}
