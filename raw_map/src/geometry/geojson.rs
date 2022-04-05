use anyhow::Result;
use geojson::Feature;

use abstutil::{Tags, Timer};
use geom::{Distance, GPSBounds, PolyLine};

use crate::geometry::{InputRoad, Results};
use crate::{osm, OriginalRoad, RawMap};

impl RawMap {
    pub fn save_osm2polygon_input(&self, output_path: String, i: osm::NodeID) -> Result<()> {
        let mut features = Vec::new();
        for id in self.roads_per_intersection(i) {
            let road = crate::initial::Road::new(self, id)?;
            let mut properties = serde_json::Map::new();
            properties.insert("osm_way_id".to_string(), id.osm_way_id.0.into());
            properties.insert("src_i".to_string(), id.i1.0.into());
            properties.insert("dst_i".to_string(), id.i2.0.into());
            properties.insert(
                "half_width".to_string(),
                road.half_width.inner_meters().into(),
            );

            let mut osm_tags = serde_json::Map::new();
            for (k, v) in road.osm_tags.inner() {
                osm_tags.insert(k.to_string(), v.to_string().into());
            }
            properties.insert("osm_tags".to_string(), osm_tags.into());

            // TODO Both for ror reading and writing, we should find a way to pair a serde struct
            // with a geo type
            features.push(Feature {
                geometry: Some(road.trimmed_center_pts.to_geojson(Some(&self.gps_bounds))),
                properties: Some(properties),
                bbox: None,
                id: None,
                foreign_members: None,
            });
        }

        // Include extra metadata as GeoJSON foreign members. They'll just show up as a top-level
        // key/values on the FeatureCollection
        let mut extra_props = serde_json::Map::new();
        extra_props.insert("intersection_id".to_string(), i.0.into());
        extra_props.insert("min_lon".to_string(), self.gps_bounds.min_lon.into());
        extra_props.insert("min_lat".to_string(), self.gps_bounds.min_lat.into());
        extra_props.insert("max_lon".to_string(), self.gps_bounds.max_lon.into());
        extra_props.insert("max_lat".to_string(), self.gps_bounds.max_lat.into());
        let fc = geojson::FeatureCollection {
            features,
            bbox: None,
            foreign_members: Some(extra_props),
        };
        let gj = geojson::GeoJson::from(fc);
        abstio::write_json(output_path, &gj);
        Ok(())
    }
}

/// Returns the (intersection_id, input roads, and GPS bounds) previously written by
/// `save_osm2polygon_input`.
pub fn read_osm2polygon_input(path: String) -> Result<(osm::NodeID, Vec<InputRoad>, GPSBounds)> {
    let geojson: geojson::GeoJson = abstio::maybe_read_json(path, &mut Timer::throwaway())?;
    if let geojson::GeoJson::FeatureCollection(collection) = geojson {
        let extra_props = collection.foreign_members.as_ref().unwrap();
        let gps_bounds = GPSBounds {
            min_lon: extra_props.get("min_lon").and_then(|x| x.as_f64()).unwrap(),
            min_lat: extra_props.get("min_lat").and_then(|x| x.as_f64()).unwrap(),
            max_lon: extra_props.get("max_lon").and_then(|x| x.as_f64()).unwrap(),
            max_lat: extra_props.get("max_lat").and_then(|x| x.as_f64()).unwrap(),
        };
        let intersection_id = osm::NodeID(
            extra_props
                .get("intersection_id")
                .and_then(|x| x.as_i64())
                .unwrap(),
        );

        let mut roads = Vec::new();
        for feature in collection.features {
            let center_pts = PolyLine::from_geojson(&feature, Some(&gps_bounds))?;
            let osm_way_id = feature
                .property("osm_way_id")
                .and_then(|x| x.as_i64())
                .unwrap();
            let src_i = feature.property("src_i").and_then(|x| x.as_i64()).unwrap();
            let dst_i = feature.property("dst_i").and_then(|x| x.as_i64()).unwrap();
            let id = OriginalRoad::new(osm_way_id, (src_i, dst_i));
            let half_width = Distance::meters(
                feature
                    .property("half_width")
                    .and_then(|x| x.as_f64())
                    .unwrap(),
            );
            let mut osm_tags = Tags::empty();
            for (k, v) in feature
                .property("osm_tags")
                .and_then(|x| x.as_object())
                .unwrap()
            {
                osm_tags.insert(k, v.as_str().unwrap());
            }

            roads.push(InputRoad {
                id,
                center_pts,
                half_width,
                osm_tags,
            });
        }

        return Ok((intersection_id, roads, gps_bounds));
    }
    bail!("No FeatureCollection")
}

impl Results {
    pub fn save_to_geojson(
        &self,
        output_path: String,
        gps_bounds: &GPSBounds,
        debug_output: bool,
    ) -> Result<()> {
        let mut features = Vec::new();

        {
            let mut properties = serde_json::Map::new();
            properties.insert("intersection_id".to_string(), self.intersection_id.0.into());
            features.push(Feature {
                geometry: Some(self.intersection_polygon.to_geojson(Some(gps_bounds))),
                properties: Some(properties),
                bbox: None,
                id: None,
                foreign_members: None,
            });
        }

        for (id, pl, half_width) in &self.trimmed_center_pts {
            // Add both a line-string and polygon per road
            let mut properties = serde_json::Map::new();
            properties.insert("osm_way_id".to_string(), id.osm_way_id.0.into());
            properties.insert("src_i".to_string(), id.i1.0.into());
            properties.insert("dst_i".to_string(), id.i2.0.into());
            features.push(Feature {
                geometry: Some(pl.to_geojson(Some(gps_bounds))),
                properties: Some(properties.clone()),
                bbox: None,
                id: None,
                foreign_members: None,
            });

            features.push(Feature {
                geometry: Some(
                    pl.make_polygons(2.0 * *half_width)
                        .to_geojson(Some(gps_bounds)),
                ),
                properties: Some(properties),
                bbox: None,
                id: None,
                foreign_members: None,
            });
        }

        if debug_output {
            for (label, polygon) in &self.debug {
                let mut properties = serde_json::Map::new();
                properties.insert("debug".to_string(), label.clone().into());
                features.push(Feature {
                    geometry: Some(polygon.to_geojson(Some(gps_bounds))),
                    properties: Some(properties),
                    bbox: None,
                    id: None,
                    foreign_members: None,
                });
            }
        }

        let fc = geojson::FeatureCollection {
            features,
            bbox: None,
            foreign_members: None,
        };
        abstio::write_json(output_path, &geojson::GeoJson::from(fc));
        Ok(())
    }
}
