use crate::make::get_lane_types;
pub use crate::make::{Hint, Hints, InitialMap};
use crate::{AreaType, IntersectionType, RoadSpec};
use geom::{GPSBounds, LonLat};
use gtfs::Route;
use serde_derive::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

// Stable IDs don't get compacted as we merge and delete things.
//#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
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

#[derive(PartialEq, Debug, Serialize, Deserialize)]
pub struct Map {
    pub roads: BTreeMap<StableRoadID, Road>,
    pub intersections: BTreeMap<StableIntersectionID, Intersection>,
    pub buildings: Vec<Building>,
    pub bus_routes: Vec<Route>,
    pub areas: Vec<Area>,

    pub boundary_polygon: Vec<LonLat>,
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
            boundary_polygon: Vec::new(),
            gps_bounds: GPSBounds::new(),
            coordinates_in_world_space: false,
        }
    }

    pub fn compute_gps_bounds(&mut self) {
        assert_eq!(self.gps_bounds, GPSBounds::new());

        for r in self.roads.values() {
            for pt in &r.points {
                self.gps_bounds.update(*pt);
            }
        }
        for i in self.intersections.values() {
            self.gps_bounds.update(i.point);
        }
        for b in &self.buildings {
            for pt in &b.points {
                self.gps_bounds.update(*pt);
            }
        }
        for a in &self.areas {
            for pt in &a.points {
                self.gps_bounds.update(*pt);
            }
        }
        for pt in &self.boundary_polygon {
            self.gps_bounds.update(*pt);
        }

        self.gps_bounds.represents_world_space = self.coordinates_in_world_space;
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
    pub num_residential_units: Option<usize>,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Area {
    pub area_type: AreaType,
    // last point is always the same as the first
    pub points: Vec<LonLat>,
    pub osm_tags: BTreeMap<String, String>,
    pub osm_id: i64,
}

// A way to refer to roads across many maps.
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct OriginalRoad {
    pub pt1: LonLat,
    pub pt2: LonLat,
}

// A way to refer to intersections across many maps.
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct OriginalIntersection {
    pub point: LonLat,
}
