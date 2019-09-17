use crate::make::get_lane_types;
pub use crate::make::{Hint, Hints, InitialMap};
use crate::{AreaType, IntersectionType, OffstreetParking, RoadSpec};
use abstutil::Timer;
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

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize, PartialOrd, Ord, Clone, Copy, Hash)]
pub struct StableBuildingID(pub usize);
impl fmt::Display for StableBuildingID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "StableBuildingID({0})", self.0)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Map {
    pub name: String,
    pub roads: BTreeMap<StableRoadID, Road>,
    pub intersections: BTreeMap<StableIntersectionID, Intersection>,
    pub buildings: BTreeMap<StableBuildingID, Building>,
    pub bus_routes: Vec<Route>,
    pub areas: Vec<Area>,
    // from OSM way => [(restriction, to OSM way)]
    pub turn_restrictions: BTreeMap<i64, Vec<(String, i64)>>,

    pub boundary_polygon: Polygon,
    pub gps_bounds: GPSBounds,
}

impl Map {
    pub fn blank(name: String) -> Map {
        Map {
            name,
            roads: BTreeMap::new(),
            intersections: BTreeMap::new(),
            buildings: BTreeMap::new(),
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
        }
    }

    pub fn find_r(&self, orig: OriginalRoad) -> Option<StableRoadID> {
        // We could quickly bail out by checking that GPSBounds contain the two points, but then
        // this breaks with valid roads that run very slightly out of bounds.
        for (id, r) in &self.roads {
            if r.orig_id.pt1.approx_eq(orig.pt1) && r.orig_id.pt2.approx_eq(orig.pt2) {
                return Some(*id);
            }
        }
        None
    }

    pub fn find_i(&self, orig: OriginalIntersection) -> Option<StableIntersectionID> {
        for (id, i) in &self.intersections {
            if i.orig_id.point.approx_eq(orig.point) {
                return Some(*id);
            }
        }
        None
    }

    pub fn apply_fixes(&mut self, fixes: &MapFixes, timer: &mut Timer) {
        let mut cnt = 0;
        for fix in &fixes.fixes {
            match fix {
                MapFix::DeleteRoad(orig) => {
                    if let Some(r) = self.find_r(*orig) {
                        self.roads.remove(&r).unwrap();
                        cnt += 1;
                    }
                }
                MapFix::DeleteIntersection(orig) => {
                    if let Some(i) = self.find_i(*orig) {
                        self.intersections.remove(&i).unwrap();
                        cnt += 1;
                    }
                }
            }
        }
        timer.note(format!("Applied {} of {} fixes ", cnt, fixes.fixes.len()));
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Road {
    // The first and last point may not match up with i1 and i2.
    pub i1: StableIntersectionID,
    pub i2: StableIntersectionID,
    // This is effectively a PolyLine, except there's a case where we need to plumb forward
    // cul-de-sac roads for roundabout handling.
    pub center_points: Vec<Pt2D>,
    pub orig_id: OriginalRoad,
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Intersection {
    // Represents the original place where OSM center-lines meet. This is meaningless beyond
    // raw_data; roads and intersections get merged and deleted.
    pub point: Pt2D,
    pub intersection_type: IntersectionType,
    pub label: Option<String>,
    pub orig_id: OriginalIntersection,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Building {
    pub polygon: Polygon,
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
    // This is needed to distinguish cul-de-sacs.
    // ... which is a bit weird, because we remove those in a later stage anyway.
    // TODO Maybe replace pt1 and pt2 with OSM node IDs? OSM node IDs may change over time
    // upstream, but as long as everything is internally consistent within A/B Street...
    pub osm_way_id: i64,
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
        let ord = self.osm_way_id.cmp(&other.osm_way_id);
        if ord != std::cmp::Ordering::Equal {
            return ord;
        }

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
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub struct OriginalIntersection {
    pub point: LonLat,
}

impl PartialOrd for OriginalIntersection {
    fn partial_cmp(&self, other: &OriginalIntersection) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Eq for OriginalIntersection {}
impl Ord for OriginalIntersection {
    fn cmp(&self, other: &OriginalIntersection) -> std::cmp::Ordering {
        // We know all the f64's are finite.
        self.point
            .longitude
            .partial_cmp(&other.point.longitude)
            .unwrap()
            .then_with(|| {
                self.point
                    .latitude
                    .partial_cmp(&other.point.latitude)
                    .unwrap()
            })
    }
}

// Directives from the synthetic crate to apply to the raw_data layer.
#[derive(Serialize, Deserialize)]
pub struct MapFixes {
    pub fixes: Vec<MapFix>,
}

impl MapFixes {
    pub fn load() -> MapFixes {
        if let Ok(f) = abstutil::read_json::<MapFixes>("../data/fixes.json") {
            f
        } else {
            MapFixes { fixes: Vec::new() }
        }
    }
}

#[derive(Serialize, Deserialize)]
pub enum MapFix {
    DeleteIntersection(OriginalIntersection),
    DeleteRoad(OriginalRoad),
}
