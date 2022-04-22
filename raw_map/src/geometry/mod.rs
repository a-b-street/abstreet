//! OSM describes roads as center-lines that intersect. Turn these into road and intersection
//! polygons roughly by
//!
//! 1) treating the road as a PolyLine with a width, so that it has a left and right edge
//! 2) finding the places where the edges of different roads intersect
//! 3) "Trimming back" the center lines to avoid the overlap
//! 4) Producing a polygon for the intersection itsef
//!
//! I wrote a novella about this: <https://a-b-street.github.io/docs/tech/map/geometry/index.html>

mod algorithm;
mod geojson;

use std::collections::BTreeMap;

use anyhow::Result;

use abstutil::Tags;
use geom::{Distance, PolyLine, Polygon};

use crate::{osm, OriginalRoad};
pub use algorithm::intersection_polygon;

#[derive(Clone)]
pub struct InputRoad {
    pub id: OriginalRoad,
    /// The true center of the road, including sidewalks. The input is untrimmed when called on the
    /// first endpoint, then trimmed on that one side when called on th second endpoint.
    pub center_pts: PolyLine,
    pub half_width: Distance,
    /// These're only used internally to decide to use some special highway on/off ramp handling.
    /// They should NOT be used for anything else, like parsing lane specs!
    pub osm_tags: Tags,
}

#[derive(Clone)]
pub struct Results {
    pub intersection_id: osm::NodeID,
    pub intersection_polygon: Polygon,
    /// Road -> (trimmed center line, half width)
    pub trimmed_center_pts: BTreeMap<OriginalRoad, (PolyLine, Distance)>,
    /// Extra polygons with labels to debug the algorithm
    pub debug: Vec<(String, Polygon)>,
}

/// Process the file produced by `save_osm2polygon_input`, then write the output as GeoJSON.
pub fn osm2polygon(input_path: String, output_path: String) -> Result<()> {
    let (intersection_id, input_roads, gps_bounds) = geojson::read_osm2polygon_input(input_path)?;
    let results = intersection_polygon(intersection_id, input_roads, &BTreeMap::new())?;
    let debug_output = false;
    results.save_to_geojson(output_path, &gps_bounds, debug_output)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_osm2polygon() {
        let mut any = false;
        for entry in std::fs::read_dir("src/geometry/tests").unwrap() {
            let input = entry.unwrap().path().display().to_string();
            println!("Working on {input}");
            if input.ends_with("output.json") {
                continue;
            }
            any = true;

            let expected_output_path = input.replace("input", "output");
            let actual_output_path = "actual_osm2polygon_output.json";
            osm2polygon(input.clone(), actual_output_path.to_string()).unwrap();
            let expected_output = std::fs::read_to_string(expected_output_path.clone()).unwrap();
            let actual_output = std::fs::read_to_string(actual_output_path).unwrap();

            if expected_output != actual_output {
                panic!("osm2polygon output changed. Manually compare {actual_output_path} and {expected_output_path}");
            }

            std::fs::remove_file(actual_output_path).unwrap();
        }
        assert!(any, "Didn't find any tests");
    }
}
