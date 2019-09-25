use crate::make::get_lane_types;
use crate::{osm, AreaType, IntersectionType, OffstreetParking, RoadSpec};
use abstutil::Timer;
use geom::{Distance, GPSBounds, Polygon, Pt2D};
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
pub struct RawMap {
    pub name: String,
    pub roads: BTreeMap<StableRoadID, RawRoad>,
    pub intersections: BTreeMap<StableIntersectionID, RawIntersection>,
    pub buildings: BTreeMap<StableBuildingID, RawBuilding>,
    pub bus_routes: Vec<Route>,
    pub areas: Vec<RawArea>,
    // from OSM way => [(restriction, to OSM way)]
    pub turn_restrictions: BTreeMap<i64, Vec<(RestrictionType, i64)>>,

    pub boundary_polygon: Polygon,
    pub gps_bounds: GPSBounds,
}

impl RawMap {
    pub fn blank(name: String) -> RawMap {
        RawMap {
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

    // TODO pub(crate) for these
    pub fn find_r(&self, orig: OriginalRoad) -> Option<StableRoadID> {
        for (id, r) in &self.roads {
            if r.orig_id == orig {
                return Some(*id);
            }
        }
        None
    }

    pub fn find_i(&self, orig: OriginalIntersection) -> Option<StableIntersectionID> {
        for (id, i) in &self.intersections {
            if i.orig_id == orig {
                return Some(*id);
            }
        }
        None
    }

    pub fn apply_fixes(&mut self, all_fixes: &BTreeMap<String, MapFixes>, timer: &mut Timer) {
        let mut dummy_fixes = MapFixes::new();

        for (name, fixes) in all_fixes {
            let mut applied = 0;
            let mut skipped = 0;

            for orig in &fixes.delete_roads {
                if let Some(r) = self.find_r(*orig) {
                    self.delete_road(r, &mut dummy_fixes);
                    applied += 1;
                } else {
                    skipped += 1;
                }
            }

            for orig in &fixes.delete_intersections {
                if let Some(i) = self.find_i(*orig) {
                    self.delete_intersection(i, &mut dummy_fixes);
                    applied += 1;
                } else {
                    skipped += 1;
                }
            }

            for i in &fixes.add_intersections {
                if self.create_intersection(i.clone()).is_some() {
                    applied += 1;
                } else {
                    skipped += 1;
                }
            }

            for r in &fixes.add_roads {
                if self.create_road(r.clone()).is_some() {
                    applied += 1;
                } else {
                    skipped += 1;
                }
            }

            for orig in &fixes.merge_short_roads {
                if let Some(r) = self.find_r(*orig) {
                    self.merge_short_road(r, &mut dummy_fixes);
                    applied += 1;
                } else {
                    skipped += 1;
                }
            }

            for (orig, osm_tags) in &fixes.override_tags {
                if let Some(r) = self.find_r(*orig) {
                    self.override_tags(r, osm_tags.clone(), &mut dummy_fixes);
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

    // TODO Might be better to maintain this instead of doing a search everytime.
    // TODO make private
    pub fn roads_per_intersection(&self, i: StableIntersectionID) -> Vec<StableRoadID> {
        let mut results = Vec::new();
        for (id, r) in &self.roads {
            if r.i1 == i || r.i2 == i {
                results.push(*id);
            }
        }
        results
    }

    pub fn new_osm_node_id(&self) -> i64 {
        // Slow, but deterministic.
        // TODO Argh, these will conflict between different maps! Is that a problem?
        let mut osm_node_id = -1;
        loop {
            if self
                .intersections
                .values()
                .any(|i| i.orig_id.osm_node_id == osm_node_id)
            {
                osm_node_id -= 1;
            } else {
                return osm_node_id;
            }
        }
    }

    pub fn new_osm_way_id(&self) -> i64 {
        // Slow, but deterministic.
        // TODO Argh, these will conflict between different maps! Is that a problem?
        let mut osm_way_id = -1;
        loop {
            if self
                .roads
                .values()
                .any(|r| r.orig_id.osm_way_id == osm_way_id)
                || self.buildings.values().any(|b| b.osm_way_id == osm_way_id)
                || self.areas.iter().any(|a| a.osm_id == osm_way_id)
            {
                osm_way_id -= 1;
            } else {
                return osm_way_id;
            }
        }
    }

    // TODO Apply the direction!
    pub fn get_turn_restrictions(&self, id: StableRoadID) -> Vec<(RestrictionType, StableRoadID)> {
        let mut results = Vec::new();
        let road = &self.roads[&id];
        if let Some(restrictions) = self.turn_restrictions.get(&road.orig_id.osm_way_id) {
            for (restriction, to) in restrictions {
                // Make sure the restriction actually applies to this road.
                if let Some(to_road) = self
                    .roads_per_intersection(road.i1)
                    .into_iter()
                    .chain(self.roads_per_intersection(road.i2))
                    .find(|r| self.roads[&r].orig_id.osm_way_id == *to)
                {
                    results.push((*restriction, to_road));
                }
            }
        }
        results
    }
}

// Mutations
impl RawMap {
    pub fn can_delete_road(&self, r: StableRoadID) -> bool {
        if !self.get_turn_restrictions(r).is_empty() {
            return false;
        }
        // Brute force search the other direction
        let osm_id = self.roads[&r].orig_id.osm_way_id;
        for restrictions in self.turn_restrictions.values() {
            for (_, to) in restrictions {
                if *to == osm_id {
                    return false;
                }
            }
        }
        true
    }

    pub fn delete_road(&mut self, r: StableRoadID, fixes: &mut MapFixes) {
        assert!(self.can_delete_road(r));
        let road = self.roads.remove(&r).unwrap();
        if !road.synthetic() {
            fixes.delete_roads.push(road.orig_id);
        }
    }

    pub fn can_delete_intersection(&self, i: StableIntersectionID) -> bool {
        self.roads_per_intersection(i).is_empty()
    }

    pub fn delete_intersection(&mut self, id: StableIntersectionID, fixes: &mut MapFixes) {
        assert!(self.can_delete_intersection(id));
        let i = self.intersections.remove(&id).unwrap();
        if !i.synthetic {
            fixes.delete_intersections.push(i.orig_id);
        }
    }

    pub fn create_intersection(&mut self, i: RawIntersection) -> Option<StableIntersectionID> {
        assert!(i.synthetic);
        if self
            .gps_bounds
            .contains(i.point.forcibly_to_gps(&self.gps_bounds))
        {
            let id = StableIntersectionID(self.intersections.keys().max().unwrap().0 + 1);
            self.intersections.insert(id, i.clone());
            Some(id)
        } else {
            None
        }
    }

    pub fn create_road(&mut self, mut r: RawRoad) -> Option<StableRoadID> {
        assert!(r.synthetic());
        match (
            self.find_i(OriginalIntersection {
                osm_node_id: r.orig_id.node1,
            }),
            self.find_i(OriginalIntersection {
                osm_node_id: r.orig_id.node2,
            }),
        ) {
            (Some(i1), Some(i2)) => {
                r.i1 = i1;
                r.i2 = i2;
                let id = StableRoadID(self.roads.keys().max().unwrap().0 + 1);
                self.roads.insert(id, r);
                Some(id)
            }
            _ => None,
        }
    }

    pub fn can_merge_short_road(&self, id: StableRoadID, fixes: &MapFixes) -> bool {
        let road = &self.roads[&id];
        let i1 = &self.intersections[&road.i1];
        let i2 = &self.intersections[&road.i2];
        if i1.intersection_type != i2.intersection_type {
            return false;
        }

        for r in self.roads_per_intersection(road.i2) {
            if self.roads[&r].synthetic() {
                return false;
            }
        }
        if i1.synthetic || i2.synthetic {
            return false;
        }
        if fixes.override_tags.contains_key(&road.orig_id) {
            return false;
        }

        true
    }

    // (the deleted intersection, list of modified roads connected to deleted intersection)
    pub fn merge_short_road(
        &mut self,
        id: StableRoadID,
        fixes: &mut MapFixes,
    ) -> Option<(StableIntersectionID, Vec<StableRoadID>)> {
        assert!(self.can_merge_short_road(id, fixes));
        let (i1, i2) = {
            let r = self.roads.remove(&id).unwrap();
            fixes.merge_short_roads.push(r.orig_id);
            (r.i1, r.i2)
        };
        let (i1_pt, i1_orig_id) = {
            let i = &self.intersections[&i1];
            (i.point, i.orig_id)
        };

        // Arbitrarily keep i1 and destroy i2.
        self.intersections.remove(&i2).unwrap();

        // Fix up all roads connected to i2.
        let mut fixed = Vec::new();
        for r in self.roads_per_intersection(i2) {
            fixed.push(r);
            let road = self.roads.get_mut(&r).unwrap();
            if road.i1 == i2 {
                road.i1 = i1;
                road.center_points[0] = i1_pt;
                // TODO Should we even do this?
                road.orig_id.node1 = i1_orig_id.osm_node_id;
            } else {
                assert_eq!(road.i2, i2);
                road.i2 = i1;
                *road.center_points.last_mut().unwrap() = i1_pt;
                // TODO Should we even do this?
                road.orig_id.node2 = i1_orig_id.osm_node_id;
            }
        }

        Some((i2, fixed))
    }

    pub fn override_tags(
        &mut self,
        r: StableRoadID,
        osm_tags: BTreeMap<String, String>,
        fixes: &mut MapFixes,
    ) {
        let road = self.roads.get_mut(&r).unwrap();
        road.osm_tags = osm_tags;
        if !road.synthetic() {
            fixes
                .override_tags
                .insert(road.orig_id, road.osm_tags.clone());
        }
    }
}

// Mutations not recorded in MapFixes yet
// TODO Fix that!
impl RawMap {
    pub fn move_intersection(
        &mut self,
        id: StableIntersectionID,
        point: Pt2D,
    ) -> Option<Vec<StableRoadID>> {
        // TODO Only for synthetic intersections, right?
        self.intersections.get_mut(&id).unwrap().point = point;

        // Update all the roads.
        let mut fixed = Vec::new();
        for r in self.roads_per_intersection(id) {
            fixed.push(r);
            let road = self.roads.get_mut(&r).unwrap();
            if road.i1 == id {
                road.center_points[0] = point;
            } else {
                assert_eq!(road.i2, id);
                *road.center_points.last_mut().unwrap() = point;
            }
        }

        Some(fixed)
    }

    pub fn modify_intersection(
        &mut self,
        id: StableIntersectionID,
        it: IntersectionType,
        label: Option<String>,
    ) {
        let i = self.intersections.get_mut(&id).unwrap();
        i.intersection_type = it;
        i.label = label;
    }

    // This shouldn't modify the endpoints, so don't have to mess around with intersections.
    pub fn override_road_points(&mut self, id: StableRoadID, pts: Vec<Pt2D>) {
        self.roads.get_mut(&id).unwrap().center_points = pts;
    }

    pub fn create_building(&mut self, bldg: RawBuilding) -> Option<StableBuildingID> {
        if bldg.polygon.center().to_gps(&self.gps_bounds).is_some() {
            let id = StableBuildingID(self.buildings.keys().max().unwrap().0 + 1);
            self.buildings.insert(id, bldg);
            Some(id)
        } else {
            None
        }
    }

    pub fn modify_building(
        &mut self,
        id: StableBuildingID,
        polygon: Polygon,
        osm_tags: BTreeMap<String, String>,
    ) {
        let bldg = self.buildings.get_mut(&id).unwrap();
        bldg.polygon = polygon;
        bldg.osm_tags = osm_tags;
    }

    pub fn delete_building(&mut self, id: StableBuildingID) {
        self.buildings.remove(&id);
    }

    pub fn delete_turn_restriction(
        &mut self,
        from: StableRoadID,
        restriction: RestrictionType,
        to: StableRoadID,
    ) {
        let to_way_id = self.roads[&to].orig_id.osm_way_id;
        let list = self
            .turn_restrictions
            .get_mut(&self.roads[&from].orig_id.osm_way_id)
            .unwrap();
        list.retain(|(r, way_id)| *r != restriction || *way_id != to_way_id);
    }

    pub fn add_turn_restriction(
        &mut self,
        from: StableRoadID,
        restriction: RestrictionType,
        to: StableRoadID,
    ) {
        self.turn_restrictions
            .entry(self.roads[&from].orig_id.osm_way_id)
            .or_insert_with(Vec::new)
            .push((restriction, self.roads[&to].orig_id.osm_way_id));
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RawRoad {
    // The first and last point may not match up with i1 and i2.
    pub i1: StableIntersectionID,
    pub i2: StableIntersectionID,
    // This is effectively a PolyLine, except there's a case where we need to plumb forward
    // cul-de-sac roads for roundabout handling.
    pub center_points: Vec<Pt2D>,
    // TODO There's redundancy between this and i1/i2 that has to be kept in sync. But removing
    // orig_id means we don't have osm_node_id embedded in MapFixes.
    pub orig_id: OriginalRoad,
    pub osm_tags: BTreeMap<String, String>,
}

impl RawRoad {
    pub fn get_spec(&self) -> RoadSpec {
        let (fwd, back) = get_lane_types(&self.osm_tags);
        RoadSpec { fwd, back }
    }

    pub fn synthetic(&self) -> bool {
        self.osm_tags.get(osm::SYNTHETIC) == Some(&"true".to_string())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RawIntersection {
    // Represents the original place where OSM center-lines meet. This is meaningless beyond
    // RawMap; roads and intersections get merged and deleted.
    pub point: Pt2D,
    pub intersection_type: IntersectionType,
    pub label: Option<String>,
    pub orig_id: OriginalIntersection,
    pub synthetic: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RawBuilding {
    pub polygon: Polygon,
    pub osm_tags: BTreeMap<String, String>,
    pub osm_way_id: i64,
    pub parking: Option<OffstreetParking>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RawArea {
    pub area_type: AreaType,
    pub polygon: Polygon,
    pub osm_tags: BTreeMap<String, String>,
    pub osm_id: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RestrictionType {
    BanTurns,
    OnlyAllowTurns,
}

impl RestrictionType {
    pub fn new(restriction: &str) -> RestrictionType {
        // Ignore the TurnType. Between two roads, there's only one category of TurnType (treating
        // Straight/LaneChangeLeft/LaneChangeRight as the same).
        //
        // Strip off time restrictions (like " @ (Mo-Fr 06:00-09:00, 15:00-18:30)")
        match restriction.split(" @ ").next().unwrap() {
            "no_left_turn" | "no_right_turn" | "no_straight_on" | "no_u_turn" | "no_anything" => {
                RestrictionType::BanTurns
            }
            "only_left_turn" | "only_right_turn" | "only_straight_on" => {
                RestrictionType::OnlyAllowTurns
            }
            _ => panic!("Unknown turn restriction {}", restriction),
        }
    }
}

// A way to refer to roads across many maps.
//
// Previously, OriginalRoad and OriginalIntersection used LonLat to reference objects across maps.
// This had some problems:
// - f64's need to be trimmed and compared carefully with epsilon checks.
// - It was confusing to modify these IDs when applying MapFixes.
// Using OSM IDs could also have problems as new OSM input is used over time, because MapFixes may
// refer to stale IDs.
// TODO Look at some stable ID standard like linear referencing
// (https://github.com/opentraffic/architecture/issues/1).
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct OriginalRoad {
    pub osm_way_id: i64,
    pub node1: i64,
    pub node2: i64,
}

// A way to refer to intersections across many maps.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct OriginalIntersection {
    pub osm_node_id: i64,
}

// Directives from the map_editor crate to apply to the RawMap layer.
#[derive(Serialize, Deserialize, Clone)]
pub struct MapFixes {
    pub delete_roads: Vec<OriginalRoad>,
    pub delete_intersections: Vec<OriginalIntersection>,
    pub add_intersections: Vec<RawIntersection>,
    pub add_roads: Vec<RawRoad>,
    pub merge_short_roads: Vec<OriginalRoad>,
    // For non-synthetic (original OSM) roads
    pub override_tags: BTreeMap<OriginalRoad, BTreeMap<String, String>>,
}

impl MapFixes {
    pub fn new() -> MapFixes {
        MapFixes {
            delete_roads: Vec::new(),
            delete_intersections: Vec::new(),
            add_intersections: Vec::new(),
            add_roads: Vec::new(),
            merge_short_roads: Vec::new(),
            override_tags: BTreeMap::new(),
        }
    }

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
