use abstutil;
use geojson::{GeoJson, PolygonType, Value};
use geom::{Bounds, LonLat, Pt2D};
use sim::Neighborhood;

pub fn convert(geojson_path: &str, map_name: String, bounds: &Bounds) {
    println!("Extracting neighborhoods from {}...", geojson_path);
    let document: GeoJson = abstutil::read_json(geojson_path).unwrap();
    match document {
        GeoJson::FeatureCollection(c) => for f in c.features.into_iter() {
            let name = f.properties.unwrap()["name"].as_str().unwrap().to_string();
            match f.geometry.unwrap().value {
                Value::Polygon(p) => {
                    convert_polygon(p, name, map_name.clone(), bounds);
                }
                Value::MultiPolygon(polygons) => for (idx, p) in polygons.into_iter().enumerate() {
                    convert_polygon(
                        p,
                        format!("{} portion #{}", name, idx + 1),
                        map_name.clone(),
                        bounds,
                    );
                },
                x => panic!("Unexpected GeoJson value {:?}", x),
            }
        },
        _ => panic!("Unexpected GeoJson root {:?}", document),
    }
}

fn convert_polygon(input: PolygonType, name: String, map_name: String, bounds: &Bounds) {
    if input.len() > 1 {
        println!("{} has a polygon with an inner ring, skipping", name);
        return;
    }

    let mut points: Vec<Pt2D> = Vec::new();
    for pt in &input[0] {
        assert_eq!(pt.len(), 2);
        let gps = LonLat::new(pt[0], pt[1]);
        if bounds.contains(gps.longitude, gps.latitude) {
            points.push(Pt2D::from_gps(&gps, bounds))
        } else {
            println!(
                "Neighborhood polygon \"{}\" is out-of-bounds, skipping",
                name
            );
            return;
        }
    }
    Neighborhood {
        map_name: map_name,
        name,
        points,
    }.save();
}
