use crate::make::get_lane_types;
use crate::{osm, AreaType, IntersectionType, RoadSpec};
use abstutil::{deserialize_btreemap, retain_btreemap, serialize_btreemap, Error, Timer, Warn};
use geom::{Angle, Distance, GPSBounds, Line, PolyLine, Polygon, Pt2D};
use gtfs::Route;
use serde_derive::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

#[derive(Debug, Serialize, Deserialize)]
pub struct RawMap {
    pub name: String,
    #[serde(
        serialize_with = "serialize_btreemap",
        deserialize_with = "deserialize_btreemap"
    )]
    pub roads: BTreeMap<OriginalRoad, RawRoad>,
    #[serde(
        serialize_with = "serialize_btreemap",
        deserialize_with = "deserialize_btreemap"
    )]
    pub intersections: BTreeMap<OriginalIntersection, RawIntersection>,
    #[serde(
        serialize_with = "serialize_btreemap",
        deserialize_with = "deserialize_btreemap"
    )]
    pub buildings: BTreeMap<OriginalBuilding, RawBuilding>,
    pub bus_routes: Vec<Route>,
    pub areas: Vec<RawArea>,

    pub boundary_polygon: Polygon,
    pub gps_bounds: GPSBounds,
    // If true, driving happens on the right side of the road (USA). If false, on the left
    // (Australia).
    pub driving_side: DrivingSide,
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
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OriginalRoad {
    pub osm_way_id: i64,
    pub i1: OriginalIntersection,
    pub i2: OriginalIntersection,
}

// A way to refer to intersections across many maps.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OriginalIntersection {
    pub osm_node_id: i64,
}

// A way to refer to buildings across many maps.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OriginalBuilding {
    pub osm_way_id: i64,
}

impl fmt::Display for OriginalRoad {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "OriginalRoad(way {} between node {} to {})",
            self.osm_way_id, self.i1.osm_node_id, self.i2.osm_node_id
        )
    }
}

impl fmt::Display for OriginalIntersection {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "OriginalIntersection({})", self.osm_node_id)
    }
}

impl fmt::Display for OriginalBuilding {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "OriginalBuilding({})", self.osm_way_id)
    }
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
            boundary_polygon: Polygon::rectangle(1.0, 1.0),
            gps_bounds: GPSBounds::new(),
            driving_side: DrivingSide::Right,
        }
    }

    pub fn apply_all_fixes(&mut self, timer: &mut Timer) {
        if self.name == "huge_seattle" {
            let master_fixes: MapFixes =
                abstutil::maybe_read_json(abstutil::path_fixes("huge_seattle"), timer)
                    .unwrap_or_else(|_| MapFixes::new(self.gps_bounds.clone()));
            self.apply_fixes("huge_seattle", &master_fixes, timer);
        } else {
            let mut master_fixes: MapFixes =
                abstutil::read_json(abstutil::path_fixes("huge_seattle"), timer);
            master_fixes.remap_pts(&self.gps_bounds);
            let local_fixes: MapFixes =
                abstutil::maybe_read_json(abstutil::path_fixes(&self.name), timer)
                    .unwrap_or_else(|_| MapFixes::new(self.gps_bounds.clone()));
            self.apply_fixes("huge_seattle", &master_fixes, timer);
            self.apply_fixes(&self.name.clone(), &local_fixes, timer);
        }

        // Validation round afterwards
        for (id, r) in &self.roads {
            if r.center_points[0] != self.intersections[&id.i1].point
                || *r.center_points.last().unwrap() != self.intersections[&id.i2].point
            {
                timer.warn(format!("{} geomtry doesn't match intersections", id));
            }
        }
    }

    fn apply_fixes(&mut self, name: &str, fixes: &MapFixes, timer: &mut Timer) {
        assert!(self.gps_bounds.approx_eq(&fixes.gps_bounds));

        let mut applied = 0;
        let mut skipped = 0;

        for r in &fixes.delete_roads {
            if self.roads.contains_key(r) {
                self.delete_road(*r);
                applied += 1;
            } else {
                skipped += 1;
            }
        }

        for i in &fixes.delete_intersections {
            if self.intersections.contains_key(i) {
                if self.can_delete_intersection(*i) {
                    self.delete_intersection(*i);
                } else {
                    println!("Can't delete {}, roads are connected, skipping", i);
                    skipped += 1;
                }
                applied += 1;
            } else {
                skipped += 1;
            }
        }

        for (id, i) in &fixes.override_intersections {
            if self
                .gps_bounds
                .contains(i.point.forcibly_to_gps(&self.gps_bounds))
            {
                self.intersections.insert(*id, i.clone());
                applied += 1;
            } else {
                skipped += 1;
            }
        }

        for (id, r) in &fixes.override_roads {
            if self.intersections.contains_key(&id.i1) && self.intersections.contains_key(&id.i2) {
                self.roads.insert(*id, r.clone());
                applied += 1;
            } else {
                skipped += 1;
            }
        }

        timer.note(format!(
            "Applied {} of {} {} fixes",
            applied,
            applied + skipped,
            name
        ));
    }

    // TODO Ignores buildings right now.
    pub fn generate_fixes(&self, timer: &mut Timer) -> MapFixes {
        let orig: RawMap = abstutil::read_binary(abstutil::path_raw_map(&self.name), timer);

        let mut fixes = MapFixes::new(self.gps_bounds.clone());

        // What'd we delete?
        fixes.delete_roads.extend(
            orig.roads
                .keys()
                .cloned()
                .collect::<BTreeSet<_>>()
                .difference(&self.roads.keys().cloned().collect::<BTreeSet<_>>()),
        );
        fixes.delete_intersections.extend(
            orig.intersections
                .keys()
                .cloned()
                .collect::<BTreeSet<_>>()
                .difference(&self.intersections.keys().cloned().collect::<BTreeSet<_>>()),
        );

        // What'd we create or modify?
        for (id, i) in &self.intersections {
            if orig
                .intersections
                .get(id)
                .map(|orig_i| orig_i != i)
                .unwrap_or(true)
            {
                fixes.override_intersections.insert(*id, i.clone());
            }
        }
        for (id, r) in &self.roads {
            if orig.roads.get(id).map(|orig_r| orig_r != r).unwrap_or(true) {
                fixes.override_roads.insert(*id, r.clone());
            }
        }

        if self.name != "huge_seattle" {
            // Filter out things that we just inherited from the master fixes.
            let mut master_fixes: MapFixes =
                abstutil::read_json(abstutil::path_fixes("huge_seattle"), timer);
            master_fixes.remap_pts(&self.gps_bounds);

            fixes.delete_roads = fixes
                .delete_roads
                .difference(&master_fixes.delete_roads)
                .cloned()
                .collect();
            fixes.delete_intersections = fixes
                .delete_intersections
                .difference(&master_fixes.delete_intersections)
                .cloned()
                .collect();
            retain_btreemap(&mut fixes.override_intersections, |id, i| {
                master_fixes
                    .override_intersections
                    .get(id)
                    .map(|orig| orig != i)
                    .unwrap_or(true)
            });
            retain_btreemap(&mut fixes.override_roads, |id, r| {
                master_fixes
                    .override_roads
                    .get(id)
                    .map(|orig| orig != r)
                    .unwrap_or(true)
            });
        }

        fixes
    }

    // TODO Might be better to maintain this instead of doing a search everytime.
    pub fn roads_per_intersection(&self, i: OriginalIntersection) -> Vec<OriginalRoad> {
        let mut results = Vec::new();
        for id in self.roads.keys() {
            if id.i1 == i || id.i2 == i {
                results.push(*id);
            }
        }
        results
    }

    pub fn new_osm_node_id(&self, start: i64) -> i64 {
        assert!(start < 0);
        // Slow, but deterministic.
        let mut osm_node_id = start;
        loop {
            if self
                .intersections
                .keys()
                .any(|i| i.osm_node_id == osm_node_id)
            {
                osm_node_id -= 1;
            } else {
                return osm_node_id;
            }
        }
    }

    pub fn new_osm_way_id(&self, start: i64) -> i64 {
        assert!(start < 0);
        // Slow, but deterministic.
        let mut osm_way_id = start;
        loop {
            if self.roads.keys().any(|r| r.osm_way_id == osm_way_id)
                || self.buildings.keys().any(|b| b.osm_way_id == osm_way_id)
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
        id: OriginalIntersection,
        timer: &mut Timer,
    ) -> (Polygon, Vec<Polygon>, Vec<(String, Polygon)>) {
        use crate::make::initial;

        let i = initial::Intersection {
            id,
            polygon: Vec::new(),
            roads: self.roads_per_intersection(id).into_iter().collect(),
            intersection_type: self.intersections[&id].intersection_type,
            elevation: self.intersections[&id].elevation,
        };
        let mut roads = BTreeMap::new();
        for r in &i.roads {
            roads.insert(*r, initial::Road::new(*r, &self.roads[r]));
        }

        let (i_pts, debug) =
            initial::intersection_polygon(self.driving_side, &i, &mut roads, timer);
        (
            Polygon::new(&i_pts),
            roads
                .values()
                .map(|r| {
                    // A little of get_thick_polyline
                    let pl = if r.fwd_width >= r.back_width {
                        self.driving_side
                            .right_shift(
                                r.trimmed_center_pts.clone(),
                                (r.fwd_width - r.back_width) / 2.0,
                            )
                            .unwrap()
                    } else {
                        self.driving_side
                            .left_shift(
                                r.trimmed_center_pts.clone(),
                                (r.back_width - r.fwd_width) / 2.0,
                            )
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
    // Return a list of turn restrictions deleted along the way.
    pub fn delete_road(&mut self, r: OriginalRoad) -> BTreeSet<TurnRestriction> {
        // First delete and warn about turn restrictions
        let restrictions = self.turn_restrictions_involving(r);
        for tr in &restrictions {
            println!(
                "Deleting {}, but first deleting turn restriction {:?} {}->{}",
                r, tr.1, tr.0, tr.2
            );
            self.delete_turn_restriction(*tr);
        }
        self.roads.remove(&r).unwrap();
        restrictions
    }

    pub fn can_delete_intersection(&self, i: OriginalIntersection) -> bool {
        self.roads_per_intersection(i).is_empty()
    }

    pub fn delete_intersection(&mut self, id: OriginalIntersection) {
        if !self.can_delete_intersection(id) {
            panic!(
                "Can't delete_intersection {}, must have roads connected",
                id
            );
        }
        self.intersections.remove(&id).unwrap();
    }

    pub fn can_merge_short_road(&self, id: OriginalRoad) -> Result<(), Error> {
        let i1 = &self.intersections[&id.i1];
        let i2 = &self.intersections[&id.i2];
        if i1.intersection_type == IntersectionType::Border
            || i2.intersection_type == IntersectionType::Border
        {
            return Err(Error::new(format!("{} touches a border", id)));
        }

        Ok(())
    }

    // (the surviving intersection, the deleted intersection, deleted roads, new roads, deleted
    // turn restrictions)
    pub fn merge_short_road(
        &mut self,
        short: OriginalRoad,
    ) -> Option<(
        OriginalIntersection,
        OriginalIntersection,
        Vec<OriginalRoad>,
        Vec<OriginalRoad>,
        BTreeSet<TurnRestriction>,
    )> {
        assert!(self.can_merge_short_road(short).is_ok());

        let mut orig_restrictions = BTreeSet::new();
        for r in self
            .roads_per_intersection(short.i1)
            .into_iter()
            .chain(self.roads_per_intersection(short.i2))
        {
            orig_restrictions.extend(self.turn_restrictions_involving(r));
        }
        // Clear out these restrictions first
        for tr in &orig_restrictions {
            println!("Warning: deleting turn restriction");
            self.delete_turn_restriction(*tr);
        }

        let (i1, i2) = (short.i1, short.i2);
        let i1_pt = self.intersections[&i1].point;

        self.roads.remove(&short).unwrap();

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

        // Fix up all roads connected to i2. Delete them and create a new copy; the ID changes,
        // since one intersection changes.
        let mut deleted = vec![short];
        let mut created = Vec::new();
        for r in self.roads_per_intersection(i2) {
            deleted.push(r);
            let mut road = self.roads.remove(&r).unwrap();
            let mut new_id = r;
            if r.i1 == i2 {
                new_id.i1 = i1;

                road.center_points[0] = i1_pt;
            // TODO More extreme: All of the points of the short road. Except there usually
            // aren't many, since they're short.
            //road.center_points.insert(0, i1_pt);
            } else {
                assert_eq!(r.i2, i2);
                new_id.i2 = i1;

                *road.center_points.last_mut().unwrap() = i1_pt;
                //road.center_points.push(i1_pt);
            }

            self.roads.insert(new_id, road);
            created.push(new_id);
        }

        Some((i1, i2, deleted, created, orig_restrictions))
    }

    pub fn can_add_turn_restriction(&self, from: OriginalRoad, to: OriginalRoad) -> bool {
        let (i1, i2) = (from.i1, from.i2);
        let (i3, i4) = (to.i1, to.i2);
        i1 == i3 || i1 == i4 || i2 == i3 || i2 == i4
    }

    fn turn_restrictions_involving(&self, r: OriginalRoad) -> BTreeSet<TurnRestriction> {
        let mut results = BTreeSet::new();
        for (tr, to) in &self.roads[&r].turn_restrictions {
            results.insert(TurnRestriction(r, *tr, *to));
        }
        for (src, road) in &self.roads {
            for (tr, to) in &road.turn_restrictions {
                if r == *to {
                    results.insert(TurnRestriction(*src, *tr, *to));
                }
            }
        }
        results
    }

    pub fn delete_turn_restriction(&mut self, tr: TurnRestriction) {
        self.roads
            .get_mut(&tr.0)
            .unwrap()
            .turn_restrictions
            .retain(|(rt, to)| tr.1 != *rt || tr.2 != *to);
    }

    pub fn move_intersection(
        &mut self,
        id: OriginalIntersection,
        point: Pt2D,
    ) -> Option<Vec<OriginalRoad>> {
        self.intersections.get_mut(&id).unwrap().point = point;

        // Update all the roads.
        let mut fixed = Vec::new();
        for r in self.roads_per_intersection(id) {
            fixed.push(r);
            let road = self.roads.get_mut(&r).unwrap();
            if r.i1 == id {
                road.center_points[0] = point;
            } else {
                assert_eq!(r.i2, id);
                *road.center_points.last_mut().unwrap() = point;
            }
        }

        Some(fixed)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RawRoad {
    // This is effectively a PolyLine, except there's a case where we need to plumb forward
    // cul-de-sac roads for roundabout handling.
    pub center_points: Vec<Pt2D>,
    pub osm_tags: BTreeMap<String, String>,
    pub turn_restrictions: Vec<(RestrictionType, OriginalRoad)>,
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
    pub elevation: Distance,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RawBuilding {
    pub polygon: Polygon,
    pub osm_tags: BTreeMap<String, String>,
    pub public_garage_name: Option<String>,
    pub num_parking_spots: usize,
    // (Name, amenity type)
    pub amenities: BTreeSet<(String, String)>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RawArea {
    pub area_type: AreaType,
    pub polygon: Polygon,
    pub osm_tags: BTreeMap<String, String>,
    pub osm_id: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RestrictionType {
    BanTurns,
    OnlyAllowTurns,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TurnRestriction(pub OriginalRoad, pub RestrictionType, pub OriginalRoad);

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

// Directives from the map_editor crate to apply to the RawMap layer.
#[derive(Serialize, Deserialize, Clone)]
pub struct MapFixes {
    // Any Pt2Ds in the rest of the fixes are relative to these GPSBounds.
    pub gps_bounds: GPSBounds,

    pub delete_roads: BTreeSet<OriginalRoad>,
    pub delete_intersections: BTreeSet<OriginalIntersection>,
    // Create or modify
    #[serde(
        serialize_with = "serialize_btreemap",
        deserialize_with = "deserialize_btreemap"
    )]
    pub override_intersections: BTreeMap<OriginalIntersection, RawIntersection>,
    #[serde(
        serialize_with = "serialize_btreemap",
        deserialize_with = "deserialize_btreemap"
    )]
    pub override_roads: BTreeMap<OriginalRoad, RawRoad>,
}

impl MapFixes {
    fn new(gps_bounds: GPSBounds) -> MapFixes {
        MapFixes {
            gps_bounds,
            delete_roads: BTreeSet::new(),
            delete_intersections: BTreeSet::new(),
            override_intersections: BTreeMap::new(),
            override_roads: BTreeMap::new(),
        }
    }

    // Only makes sense to call for huge_seattle fixes
    fn remap_pts(&mut self, local_gps_bounds: &GPSBounds) {
        let master_gps_bounds = &self.gps_bounds;
        assert!(!master_gps_bounds.approx_eq(local_gps_bounds));
        for i in self.override_intersections.values_mut() {
            i.point = Pt2D::forcibly_from_gps(
                i.point.to_gps(master_gps_bounds).unwrap(),
                local_gps_bounds,
            );
        }

        for mut r in self.override_roads.values_mut() {
            r.center_points = local_gps_bounds
                .forcibly_convert(&master_gps_bounds.must_convert_back(&r.center_points));
        }

        self.gps_bounds = local_gps_bounds.clone();
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq)]
pub enum DrivingSide {
    Right,
    Left,
}

impl DrivingSide {
    // "right" and "left" here are in terms of DrivingSide::Right, what I'm used to reasoning about
    // in the USA. They invert appropriately for DrivingSide::Left.
    pub fn right_shift(self, pl: PolyLine, width: Distance) -> Warn<PolyLine> {
        match self {
            DrivingSide::Right => pl.shift_right(width),
            DrivingSide::Left => pl.shift_left(width),
        }
    }

    pub fn left_shift(self, pl: PolyLine, width: Distance) -> Warn<PolyLine> {
        match self {
            DrivingSide::Right => pl.shift_left(width),
            DrivingSide::Left => pl.shift_right(width),
        }
    }

    pub fn right_shift_line(self, line: Line, width: Distance) -> Line {
        match self {
            DrivingSide::Right => line.shift_right(width),
            DrivingSide::Left => line.shift_left(width),
        }
    }

    pub fn left_shift_line(self, line: Line, width: Distance) -> Line {
        match self {
            DrivingSide::Right => line.shift_left(width),
            DrivingSide::Left => line.shift_right(width),
        }
    }

    pub fn angle_offset(self, a: Angle) -> Angle {
        match self {
            DrivingSide::Right => a,
            DrivingSide::Left => a.opposite(),
        }
    }
}
