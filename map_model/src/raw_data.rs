use crate::make::get_lane_types;
pub use crate::make::InitialMap;
use crate::{AreaType, IntersectionType, RoadSpec};
use geom::{Distance, GPSBounds, LonLat};
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
    pub parcels: Vec<Parcel>,
    pub bus_routes: Vec<Route>,
    pub areas: Vec<Area>,

    pub boundary_polygon: Vec<LonLat>,
    pub coordinates_in_world_space: bool,
}

impl Map {
    pub fn blank() -> Map {
        Map {
            roads: BTreeMap::new(),
            intersections: BTreeMap::new(),
            buildings: Vec::new(),
            parcels: Vec::new(),
            bus_routes: Vec::new(),
            areas: Vec::new(),
            boundary_polygon: Vec::new(),
            coordinates_in_world_space: false,
        }
    }

    pub fn get_gps_bounds(&self) -> GPSBounds {
        let mut bounds = GPSBounds::new();

        for r in self.roads.values() {
            for pt in &r.points {
                bounds.update(*pt);
            }
        }
        for i in self.intersections.values() {
            bounds.update(i.point);
        }
        for b in &self.buildings {
            for pt in &b.points {
                bounds.update(*pt);
            }
        }
        for a in &self.areas {
            for pt in &a.points {
                bounds.update(*pt);
            }
        }
        for p in &self.parcels {
            for pt in &p.points {
                bounds.update(*pt);
            }
        }
        for pt in &self.boundary_polygon {
            bounds.update(*pt);
        }

        bounds.represents_world_space = self.coordinates_in_world_space;

        bounds
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
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Intersection {
    pub point: LonLat,
    pub elevation: Distance,
    pub intersection_type: IntersectionType,
    pub label: Option<String>,
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

#[derive(PartialEq, Debug, Serialize, Deserialize)]
pub struct Parcel {
    // last point never the first?
    pub points: Vec<LonLat>,
    // TODO decide what metadata from the shapefile is useful
    pub block: usize,
}
