use crate::make::get_lane_types;
pub use crate::make::{remove_disconnected_roads, Hint, Hints, InitialMap};
use crate::{AreaType, IntersectionType, OffstreetParking, RoadSpec};
use abstutil::Timer;
use geom::{Distance, GPSBounds, LonLat, Polygon, Pt2D};
use gtfs::Route;
use serde_derive::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
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

    pub fn apply_fixes(&mut self, all_fixes: &BTreeMap<String, MapFixes>, timer: &mut Timer) {
        for (name, fixes) in all_fixes {
            let mut applied = 0;
            let mut skipped = 0;

            for orig in &fixes.delete_roads {
                if let Some(r) = self.find_r(*orig) {
                    self.roads.remove(&r).unwrap();
                    applied += 1;
                } else {
                    skipped += 1;
                }
            }

            for orig in &fixes.delete_intersections {
                if let Some(i) = self.find_i(*orig) {
                    self.intersections.remove(&i).unwrap();
                    applied += 1;
                } else {
                    skipped += 1;
                }
            }

            for i in &fixes.add_intersections {
                if self.gps_bounds.contains(i.orig_id.point) {
                    let id = StableIntersectionID(self.intersections.keys().max().unwrap().0 + 1);
                    self.intersections.insert(id, i.clone());
                    applied += 1;
                } else {
                    skipped += 1;
                }
            }

            for r in &fixes.add_roads {
                match (
                    self.find_i(OriginalIntersection {
                        point: r.orig_id.pt1,
                    }),
                    self.find_i(OriginalIntersection {
                        point: r.orig_id.pt2,
                    }),
                ) {
                    (Some(i1), Some(i2)) => {
                        let mut road = r.clone();
                        road.i1 = i1;
                        road.i2 = i2;
                        let id = StableRoadID(self.roads.keys().max().unwrap().0 + 1);
                        self.roads.insert(id, road);
                        applied += 1;
                    }
                    _ => {
                        skipped += 1;
                    }
                }
            }

            for orig in &fixes.merge_short_roads {
                if let Some(id) = self.find_r(*orig) {
                    // TODO Whoa, this one is complicated!

                    let (i1, i2) = {
                        let r = self.roads.remove(&id).unwrap();
                        (r.i1, r.i2)
                    };
                    let (i1_pt, i1_orig_id_pt) = {
                        let i = &self.intersections[&i1];
                        (i.point, i.orig_id.point)
                    };

                    // Arbitrarily keep i1 and destroy i2.
                    // TODO Make sure intersection types are the same. Make sure i2 isn't synthetic.
                    self.intersections.remove(&i2).unwrap();

                    // Fix up all roads connected to i2.
                    // TODO Maintain roads_per_intersection in raw_data? This will get slow.
                    for r in self.roads.values_mut() {
                        if r.i1 == i2 {
                            r.i1 = i1;
                            r.center_points[0] = i1_pt;
                            r.orig_id.pt1 = i1_orig_id_pt;
                        } else if r.i2 == i2 {
                            r.i2 = i1;
                            *r.center_points.last_mut().unwrap() = i1_pt;
                            r.orig_id.pt2 = i1_orig_id_pt;
                        }
                    }

                    applied += 1;
                } else {
                    skipped += 1;
                }
            }

            for (orig, osm_tags) in &fixes.override_tags {
                if let Some(r) = self.find_r(*orig) {
                    self.roads.get_mut(&r).unwrap().osm_tags = osm_tags.clone();
                    applied += 1;
                } else {
                    skipped += 1;
                }
            }

            timer.note(format!(
                "Applied {} of {} fixes for {}",
                applied,
                applied + skipped,
                name
            ));
        }
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
}

impl Road {
    pub fn get_spec(&self) -> RoadSpec {
        let (fwd, back) = get_lane_types(&self.osm_tags);
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
    pub synthetic: bool,
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
#[derive(Serialize, Deserialize, Clone)]
pub struct MapFixes {
    pub delete_roads: Vec<OriginalRoad>,
    pub delete_intersections: Vec<OriginalIntersection>,
    pub add_intersections: Vec<Intersection>,
    pub add_roads: Vec<Road>,
    pub merge_short_roads: Vec<OriginalRoad>,
    // For non-synthetic (original OSM) roads
    pub override_tags: BTreeMap<OriginalRoad, BTreeMap<String, String>>,
}

impl MapFixes {
    // The groups of fixes should be applicable in any order, theoretically...
    pub fn load(timer: &mut Timer) -> BTreeMap<String, MapFixes> {
        // Make sure different groups of fixes don't conflict.
        let mut seen_roads = BTreeSet::new();
        let mut seen_intersections = BTreeSet::new();

        let mut results = BTreeMap::new();
        for name in abstutil::list_all_objects("fixes", "") {
            let fixes: MapFixes = abstutil::read_json(&abstutil::path_fixes(&name), timer).unwrap();
            let (new_roads, new_intersections) = fixes.all_touched_ids();
            if !seen_roads.is_disjoint(&new_roads) {
                // The error could be much better (which road and other MapFixes), but since we
                // guard against this happening in the first place, don't bother.
                panic!(
                    "{} MapFixes and some other MapFixes both touch the same road!",
                    name
                );
            }
            seen_roads.extend(new_roads);
            if !seen_intersections.is_disjoint(&new_intersections) {
                panic!(
                    "{} MapFixes and some other MapFixes both touch the same intersection!",
                    name
                );
            }
            seen_intersections.extend(new_intersections);

            results.insert(name, fixes);
        }
        results
    }

    pub fn all_touched_ids(&self) -> (BTreeSet<OriginalRoad>, BTreeSet<OriginalIntersection>) {
        let mut roads: BTreeSet<OriginalRoad> = self.delete_roads.iter().cloned().collect();
        for r in &self.add_roads {
            roads.insert(r.orig_id);
        }
        roads.extend(self.merge_short_roads.clone());
        roads.extend(self.override_tags.keys().cloned());

        let mut intersections: BTreeSet<OriginalIntersection> =
            self.delete_intersections.iter().cloned().collect();
        for i in &self.add_intersections {
            intersections.insert(i.orig_id);
        }

        (roads, intersections)
    }
}
