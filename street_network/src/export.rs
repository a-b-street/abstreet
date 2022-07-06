use std::fs::File;
use std::io::Write;
use std::path::Path;

use anyhow::Result;
use geojson::Feature;

use abstutil::Timer;

use crate::StreetNetwork;

impl StreetNetwork {
    /// Assumes `run_all_simplifications` has been called if desired
    pub fn save_to_geojson(&self, output_path: String, timer: &mut Timer) -> Result<()> {
        // TODO InitialMap is going away very soon, but we still need it
        let initial_map =
            crate::initial::InitialMap::new(self, &self.gps_bounds.to_bounds(), timer);

        let mut features = Vec::new();

        // Add a line-string and polygon per road
        for (id, road) in &initial_map.roads {
            let mut properties = serde_json::Map::new();
            properties.insert("osm_way_id".to_string(), id.osm_way_id.0.into());
            properties.insert("src_i".to_string(), id.i1.0.into());
            properties.insert("dst_i".to_string(), id.i2.0.into());
            features.push(Feature {
                geometry: Some(road.trimmed_center_pts.to_geojson(Some(&self.gps_bounds))),
                properties: Some(properties.clone()),
                bbox: None,
                id: None,
                foreign_members: None,
            });

            features.push(Feature {
                geometry: Some(
                    road.trimmed_center_pts
                        .make_polygons(2.0 * road.half_width)
                        .to_geojson(Some(&self.gps_bounds)),
                ),
                properties: Some(properties),
                bbox: None,
                id: None,
                foreign_members: None,
            });
        }

        // Polygon per intersection
        for (id, intersection) in &initial_map.intersections {
            let mut properties = serde_json::Map::new();
            properties.insert("intersection_id".to_string(), id.0.into());
            // Just some styling for geojson.io to distinguish roads/intersections better
            properties.insert("fill".to_string(), "#729fcf".into());
            features.push(Feature {
                geometry: Some(intersection.polygon.to_geojson(Some(&self.gps_bounds))),
                properties: Some(properties),
                bbox: None,
                id: None,
                foreign_members: None,
            });
        }

        let fc = geojson::FeatureCollection {
            features,
            bbox: None,
            foreign_members: None,
        };
        let obj = geojson::GeoJson::from(fc);

        std::fs::create_dir_all(Path::new(&output_path).parent().unwrap())?;
        let mut file = File::create(output_path)?;
        file.write_all(serde_json::to_string_pretty(&obj)?.as_bytes())?;
        Ok(())
    }
}
