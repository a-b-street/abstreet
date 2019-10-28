use crate::make::get_lane_types;
use crate::{osm, AreaType, IntersectionType, OffstreetParking, RoadSpec};
use abstutil::{Error, Timer};
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
            // Some nonsense thing
            boundary_polygon: Polygon::rectangle(
                Pt2D::new(50.0, 50.0),
                Distance::meters(1.0),
                Distance::meters(1.0),
            ),
            gps_bounds: GPSBounds::new(),
        }
    }

    fn find_r(&self, orig: OriginalRoad) -> Option<StableRoadID> {
        for (id, r) in &self.roads {
            if r.orig_id == orig {
                return Some(*id);
            }
        }
        None
    }

    fn find_i(&self, orig: OriginalIntersection) -> Option<StableIntersectionID> {
        for (id, i) in &self.intersections {
            if i.orig_id == orig {
                return Some(*id);
            }
        }
        None
    }

    pub fn apply_fixes(&mut self, all_fixes: &BTreeMap<String, MapFixes>, timer: &mut Timer) {
        timer.start("applying all fixes");
        for (name, fixes) in all_fixes {
            let mut applied = 0;
            let mut skipped = 0;

            for orig in &fixes.delete_roads {
                if let Some(r) = self.find_r(*orig) {
                    self.delete_road(r);
                    applied += 1;
                } else {
                    skipped += 1;
                }
            }

            for orig in &fixes.delete_intersections {
                if let Some(i) = self.find_i(*orig) {
                    self.delete_intersection(i);
                    applied += 1;
                } else {
                    skipped += 1;
                }
            }

            let remap_pts = !self.gps_bounds.approx_eq(&fixes.gps_bounds);

            for mut i in fixes.override_intersections.clone() {
                if remap_pts {
                    i.point = Pt2D::forcibly_from_gps(
                        i.point.to_gps(&fixes.gps_bounds).unwrap(),
                        &self.gps_bounds,
                    );
                }

                if self.override_intersection(i).is_some() {
                    applied += 1;
                } else {
                    skipped += 1;
                }
            }

            for mut r in fixes.override_roads.clone() {
                if remap_pts {
                    r.center_points = self
                        .gps_bounds
                        .forcibly_convert(&fixes.gps_bounds.must_convert_back(&r.center_points));
                }

                if self.override_road(r).is_some() {
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
        timer.stop("applying all fixes");
    }

    // TODO Might be better to maintain this instead of doing a search everytime.
    pub fn roads_per_intersection(&self, i: StableIntersectionID) -> Vec<StableRoadID> {
        let mut results = Vec::new();
        for (id, r) in &self.roads {
            if r.i1 == i || r.i2 == i {
                results.push(*id);
            }
        }
        results
    }

    pub fn new_osm_node_id(&self, start: i64) -> i64 {
        // Slow, but deterministic.
        let mut osm_node_id = start;
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

    pub fn new_osm_way_id(&self, start: i64) -> i64 {
        // Slow, but deterministic.
        let mut osm_way_id = start;
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

    // (Intersection polygon, polygons for roads, list of labeled polylines to debug)
    pub fn preview_intersection(
        &self,
        id: StableIntersectionID,
        timer: &mut Timer,
    ) -> (Polygon, Vec<Polygon>, Vec<(String, Polygon)>) {
        use crate::make::initial;

        let i = initial::Intersection {
            id,
            polygon: Vec::new(),
            roads: self.roads_per_intersection(id).into_iter().collect(),
            intersection_type: self.intersections[&id].intersection_type,
        };
        let mut roads = BTreeMap::new();
        for r in &i.roads {
            roads.insert(*r, initial::Road::new(*r, &self.roads[r]));
        }

        let (i_pts, debug) = initial::intersection_polygon(&i, &mut roads, timer);
        (
            Polygon::new(&i_pts),
            roads
                .values()
                .map(|r| {
                    // A little of get_thick_polyline
                    let pl = if r.fwd_width >= r.back_width {
                        r.trimmed_center_pts
                            .shift_right((r.fwd_width - r.back_width) / 2.0)
                            .unwrap()
                    } else {
                        r.trimmed_center_pts
                            .shift_left((r.back_width - r.fwd_width) / 2.0)
                            .unwrap()
                    };
                    pl.make_polygons(r.fwd_width + r.back_width)
                })
                .collect(),
            debug,
        )
    }
}

// Mutations and supporting queries
impl RawMap {
    pub fn delete_road(&mut self, r: StableRoadID) {
        // First delete and warn about turn restrictions
        if !self.roads[&r].turn_restrictions.is_empty() {
            println!("Deleting {}, but note it has turn restrictions from it", r);
        }
        // Brute force search the other direction
        let mut cleanup_other_roads = Vec::new();
        for (src, road) in &self.roads {
            for (_, to) in &road.turn_restrictions {
                if r == *to {
                    println!(
                        "Deleting turn restriction from other road {} to {}",
                        src, to
                    );
                    cleanup_other_roads.push(*src);
                }
            }
        }
        for src in cleanup_other_roads {
            self.roads
                .get_mut(&src)
                .unwrap()
                .turn_restrictions
                .retain(|(_, to)| *to != r);
        }

        self.roads.remove(&r).unwrap();
    }

    pub fn can_delete_intersection(&self, i: StableIntersectionID) -> bool {
        self.roads_per_intersection(i).is_empty()
    }

    pub fn delete_intersection(&mut self, id: StableIntersectionID) {
        if !self.can_delete_intersection(id) {
            panic!(
                "Can't delete_intersection {:?}, must have roads connected",
                self.intersections[&id].orig_id
            );
        }
        self.intersections.remove(&id).unwrap();
    }

    // These two are kind of just for apply_fixes, except they're also useful to create new stuff,
    // since they allocate a new ID.
    pub fn override_intersection(&mut self, i: RawIntersection) -> Option<StableIntersectionID> {
        // Existing?
        if let Some(id) = self.find_i(i.orig_id) {
            self.intersections.insert(id, i);
            return Some(id);
        }

        // New
        if self
            .gps_bounds
            .contains(i.point.forcibly_to_gps(&self.gps_bounds))
        {
            let id = StableIntersectionID(self.intersections.keys().max().unwrap().0 + 1);
            self.intersections.insert(id, i);
            Some(id)
        } else {
            None
        }
    }

    pub fn override_road(&mut self, mut r: RawRoad) -> Option<StableRoadID> {
        // Existing?
        if let Some(id) = self.find_r(r.orig_id) {
            // TODO Do we need to rewrite i1 and i2?
            self.roads.insert(id, r);
            return Some(id);
        }

        // New
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

    pub fn can_merge_short_road(&self, id: StableRoadID) -> Result<(), Error> {
        let road = &self.roads[&id];
        let i1 = &self.intersections[&road.i1];
        let i2 = &self.intersections[&road.i2];
        if i1.intersection_type == IntersectionType::Border
            || i2.intersection_type == IntersectionType::Border
        {
            return Err(Error::new(format!("{} touches a border", id)));
        }

        Ok(())
    }

    // (the surviving intersection, the deleted intersection, list of modified roads connected to
    // deleted intersection)
    pub fn merge_short_road(
        &mut self,
        id: StableRoadID,
    ) -> Option<(
        StableIntersectionID,
        StableIntersectionID,
        Vec<StableRoadID>,
    )> {
        assert!(self.can_merge_short_road(id).is_ok());
        let (i1, i2) = {
            let r = &self.roads[&id];
            (r.i1, r.i2)
        };
        let (i1_pt, i1_orig_id) = {
            let i = &self.intersections[&i1];
            (i.point, i.orig_id)
        };

        self.delete_road(id);

        // Arbitrarily keep i1 and destroy i2. If the intersection types differ, upgrade the
        // surviving interesting.
        {
            // Don't use delete_intersection; we're manually fixing up connected roads
            let i = self.intersections.remove(&i2).unwrap();
            if i.intersection_type == IntersectionType::TrafficSignal {
                self.intersections.get_mut(&i1).unwrap().intersection_type =
                    IntersectionType::TrafficSignal;
            }
        }

        // Fix up all roads connected to i2.
        let mut fixed = Vec::new();
        for r in self.roads_per_intersection(i2) {
            fixed.push(r);
            let road = self.roads.get_mut(&r).unwrap();
            if road.i1 == i2 {
                road.i1 = i1;

                road.center_points[0] = i1_pt;
                // TODO More extreme: All of the points of the short road. Except there usually
                // aren't many, since they're short.
                //road.center_points.insert(0, i1_pt);

                // TODO Should we even do this?
                road.orig_id.node1 = i1_orig_id.osm_node_id;
            } else {
                assert_eq!(road.i2, i2);
                road.i2 = i1;

                *road.center_points.last_mut().unwrap() = i1_pt;
                //road.center_points.push(i1_pt);

                // TODO Should we even do this?
                road.orig_id.node2 = i1_orig_id.osm_node_id;
            }
        }

        Some((i1, i2, fixed))
    }

    pub fn can_add_turn_restriction(&self, from: StableRoadID, to: StableRoadID) -> bool {
        let (i1, i2) = {
            let r = &self.roads[&from];
            (r.i1, r.i2)
        };
        let (i3, i4) = {
            let r = &self.roads[&to];
            (r.i1, r.i2)
        };
        i1 == i3 || i1 == i4 || i2 == i3 || i2 == i4
    }

    pub fn move_intersection(
        &mut self,
        id: StableIntersectionID,
        point: Pt2D,
    ) -> Option<Vec<StableRoadID>> {
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
                // TODO Don't update orig_id, right?
            }
        }

        Some(fixed)
    }

    // TODO Some of these are pretty silly

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

    pub fn generate_fixes(&self, fixes_name: &str, timer: &mut Timer) -> MapFixes {
        let orig: RawMap =
            abstutil::read_binary(&abstutil::path_raw_map(&self.name), timer).unwrap();

        let mut fixes = MapFixes {
            gps_bounds: self.gps_bounds.clone(),
            delete_roads: Vec::new(),
            delete_intersections: Vec::new(),
            override_intersections: Vec::new(),
            override_roads: Vec::new(),
        };

        // What'd we delete?
        for r in orig.roads.values() {
            if self.find_r(r.orig_id).is_none() {
                fixes.delete_roads.push(r.orig_id);
            }
        }
        for i in orig.intersections.values() {
            if self.find_i(i.orig_id).is_none() {
                fixes.delete_intersections.push(i.orig_id);
            }
        }

        // What'd we create or modify?
        for i in self.intersections.values() {
            if orig
                .find_i(i.orig_id)
                .map(|id| orig.intersections[&id] != *i)
                .unwrap_or(true)
            {
                fixes.override_intersections.push(i.clone());
            }
        }
        for r in self.roads.values() {
            if orig
                .find_r(r.orig_id)
                .map(|id| orig.roads[&id] != *r)
                .unwrap_or(true)
            {
                fixes.override_roads.push(r.clone());
            }
        }

        // Filter out things from other fixes.
        // TODO If we accidentally modify something from another set of fixes, then we silently
        // discard that change. Oops!
        let mut seen_roads = BTreeSet::new();
        let mut seen_intersections = BTreeSet::new();
        for name in abstutil::list_all_objects("fixes", "") {
            if name == fixes_name {
                continue;
            }
            let f: MapFixes = abstutil::read_json(&abstutil::path_fixes(&name), timer).unwrap();
            let (new_roads, new_intersections) = f.all_touched_ids();
            seen_roads.extend(new_roads);
            seen_intersections.extend(new_intersections);
        }

        fixes.delete_roads.retain(|r| !seen_roads.contains(r));
        fixes
            .delete_intersections
            .retain(|i| !seen_intersections.contains(i));
        fixes
            .override_intersections
            .retain(|i| !seen_intersections.contains(&i.orig_id));
        fixes
            .override_roads
            .retain(|r| !seen_roads.contains(&r.orig_id));

        fixes
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
    pub turn_restrictions: Vec<(RestrictionType, StableRoadID)>,
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
    // Any Pt2Ds in the rest of the fixes are relative to these GPSBounds.
    pub gps_bounds: GPSBounds,

    pub delete_roads: Vec<OriginalRoad>,
    pub delete_intersections: Vec<OriginalIntersection>,
    // Create or modify
    pub override_intersections: Vec<RawIntersection>,
    pub override_roads: Vec<RawRoad>,
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

    fn all_touched_ids(&self) -> (BTreeSet<OriginalRoad>, BTreeSet<OriginalIntersection>) {
        let mut roads: BTreeSet<OriginalRoad> = self.delete_roads.iter().cloned().collect();
        for r in &self.override_roads {
            roads.insert(r.orig_id);
        }

        let mut intersections: BTreeSet<OriginalIntersection> =
            self.delete_intersections.iter().cloned().collect();
        for i in &self.override_intersections {
            intersections.insert(i.orig_id);
        }

        (roads, intersections)
    }
}
