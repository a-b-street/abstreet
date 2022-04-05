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

use std::collections::{BTreeMap, BTreeSet};

use anyhow::Result;

use abstutil::Tags;
use geom::{Distance, PolyLine, Polygon};

use crate::initial::Road;
use crate::{osm, OriginalRoad};
pub use algorithm::intersection_polygon;

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

pub struct Results {
    pub intersection_id: osm::NodeID,
    pub intersection_polygon: Polygon,
    /// (Road, trimmed center line, half width)
    pub trimmed_center_pts: Vec<(OriginalRoad, PolyLine, Distance)>,
    /// Extra polygons with labels to debug the algorithm
    pub debug: Vec<(String, Polygon)>,
}

/// Process the file produced by `save_osm2polygon_input`, then write the output as GeoJSON.
pub fn osm2polygon(input_path: String, output_path: String) -> Result<()> {
    let (intersection_id, input_roads, gps_bounds) = geojson::read_osm2polygon_input(input_path)?;
    let results = intersection_polygon_v2(intersection_id, input_roads)?;
    let debug_output = false;
    results.save_to_geojson(output_path, &gps_bounds, debug_output)?;
    Ok(())
}

fn intersection_polygon_v2(
    intersection_id: osm::NodeID,
    input_roads: Vec<InputRoad>,
) -> Result<Results> {
    let mut intersection_roads = BTreeSet::new();
    let mut roads = BTreeMap::new();
    for road in input_roads {
        intersection_roads.insert(road.id);
        roads.insert(
            road.id,
            Road {
                id: road.id,
                src_i: road.id.i1,
                dst_i: road.id.i2,
                trimmed_center_pts: road.center_pts,
                half_width: road.half_width,
                osm_tags: road.osm_tags,
                // Unused
                lane_specs_ltr: Vec::new(),
            },
        );
    }

    let (intersection_polygon, debug) = intersection_polygon(
        intersection_id,
        intersection_roads,
        &mut roads,
        // No trim_roads_for_merging
        &BTreeMap::new(),
    )?;

    let trimmed_center_pts = roads
        .into_values()
        .map(|road| (road.id, road.trimmed_center_pts, road.half_width))
        .collect();
    let result = Results {
        intersection_id,
        intersection_polygon,
        trimmed_center_pts,
        debug,
    };
    Ok(result)
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
