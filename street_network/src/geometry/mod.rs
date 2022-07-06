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

use std::collections::BTreeMap;

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
