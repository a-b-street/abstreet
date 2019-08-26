use crate::make::get_lane_types;
pub use crate::make::{Hint, Hints, InitialMap};
use crate::{AreaType, IntersectionType, OffstreetParking, RoadSpec};
use geom::{Distance, GPSBounds, LonLat, Polygon, Pt2D};
use gtfs::Route;
use serde_derive::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

// Stable IDs don't get compacted as we merge and delete things.
#[derive(PartialEq, Eq, Debug, Serialize, Deserialize, PartialOrd, Ord, Clone, Copy, Hash)]
pub struct StableRoadID(pub usize);
impl fmt::Display for StableRoadID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "StableRoadID({0})", self.0)
    }
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize, PartialOrd, Ord, Clone, Copy, Hash)]
pub struct StableIntersectionID(pub usize);
impl fmt::Display for StableIntersectionID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "StableIntersectionID({0})", self.0)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Map {
    pub roads: BTreeMap<StableRoadID, Road>,
    pub intersections: BTreeMap<StableIntersectionID, Intersection>,
    pub buildings: Vec<Building>,
    pub bus_routes: Vec<Route>,
    pub areas: Vec<Area>,
    // from OSM way => [(restriction, to OSM way)]
    pub turn_restrictions: BTreeMap<i64, Vec<(String, i64)>>,

    pub boundary_polygon: Polygon,
    pub gps_bounds: GPSBounds,
    pub coordinates_in_world_space: bool,
}

impl Map {
    pub fn blank() -> Map {
        Map {
            roads: BTreeMap::new(),
            intersections: BTreeMap::new(),
            buildings: Vec::new(),
            bus_routes: Vec::new(),
            areas: Vec::new(),
            turn_restrictions: BTreeMap::new(),
            // Some nonsense thing
            boundary_polygon: Polygon::rectangle(
                Pt2D::new(50.0, 50.0),
                Distance::meters(1.0),
                Distance::meters(1.0),
            ),
            gps_bounds: GPSBounds::new(),
            coordinates_in_world_space: false,
        }
    }

    pub fn find_r(&self, orig: OriginalRoad) -> Option<StableRoadID> {
        if !self.gps_bounds.contains(orig.pt1) || !self.gps_bounds.contains(orig.pt2) {
            return None;
        }
        for (id, r) in &self.roads {
            if r.points[0] == orig.pt1 && *r.points.last().unwrap() == orig.pt2 {
                return Some(*id);
            }
        }

        // There will be cases where the point fits in the bounding box, but isn't inside the
        // clipping polygon.
        None
    }

    pub fn find_i(&self, orig: OriginalIntersection) -> Option<StableIntersectionID> {
        if !self.gps_bounds.contains(orig.point) {
            return None;
        }
        for (id, i) in &self.intersections {
            if i.point == orig.point {
                return Some(*id);
            }
        }

        // TODO There will be cases where the point fits in the bounding box, but isn't inside the
        // clipping polygon.
        None
    }
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Road {
    // The first and last point may not match up with i1 and i2.
    pub i1: StableIntersectionID,
    pub i2: StableIntersectionID,
    pub points: Vec<LonLat>,
    pub osm_tags: BTreeMap<String, String>,
    pub osm_way_id: i64,
    pub parking_lane_fwd: bool,
    pub parking_lane_back: bool,
}

impl Road {
    pub fn get_spec(&self) -> RoadSpec {
        let (fwd, back) = get_lane_types(
            &self.osm_tags,
            self.parking_lane_fwd,
            self.parking_lane_back,
        );
        RoadSpec { fwd, back }
    }

    pub fn orig_id(&self) -> OriginalRoad {
        OriginalRoad {
            pt1: self.points[0],
            pt2: *self.points.last().unwrap(),
        }
    }
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Intersection {
    // Represents the original place where OSM center-lines meet. This is meaningless beyond
    // raw_data; roads and intersections get merged and deleted.
    pub point: LonLat,
    pub intersection_type: IntersectionType,
    pub label: Option<String>,
}

impl Intersection {
    pub fn orig_id(&self) -> OriginalIntersection {
        OriginalIntersection { point: self.point }
    }
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Building {
    // last point never the first?
    pub points: Vec<LonLat>,
    pub osm_tags: BTreeMap<String, String>,
    pub osm_way_id: i64,
    pub parking: Option<OffstreetParking>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Area {
    pub area_type: AreaType,
    pub polygon: Polygon,
    pub osm_tags: BTreeMap<String, String>,
    pub osm_id: i64,
}

// A way to refer to roads across many maps.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub struct OriginalRoad {
    pub pt1: LonLat,
    pub pt2: LonLat,
}

// Since we don't do arithmetic on the original LonLat's, it's reasonably safe to declare these Eq
// and Ord.
impl PartialOrd for OriginalRoad {
    fn partial_cmp(&self, other: &OriginalRoad) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Eq for OriginalRoad {}
impl Ord for OriginalRoad {
    fn cmp(&self, other: &OriginalRoad) -> std::cmp::Ordering {
        // We know all the f64's are finite. then_with() produces ugly nesting, so manually do it.
        let ord = self
            .pt1
            .longitude
            .partial_cmp(&other.pt1.longitude)
            .unwrap();
        if ord != std::cmp::Ordering::Equal {
            return ord;
        }
        let ord = self.pt1.latitude.partial_cmp(&other.pt1.latitude).unwrap();
        if ord != std::cmp::Ordering::Equal {
            return ord;
        }
        let ord = self
            .pt2
            .longitude
            .partial_cmp(&other.pt2.longitude)
            .unwrap();
        if ord != std::cmp::Ordering::Equal {
            return ord;
        }
        self.pt2.latitude.partial_cmp(&other.pt2.latitude).unwrap()
    }
}

// A way to refer to intersections across many maps.
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct OriginalIntersection {
    pub point: LonLat,
}
