//! The convert_osm crate produces a RawMap from OSM and other data. Storing this intermediate
//! structure is useful to iterate quickly on parts of the map importing pipeline without having to
//! constantly read .osm files, and to visualize the intermediate state with map_editor.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use anyhow::{Context, Result};
use petgraph::graphmap::DiGraphMap;
use serde::{Deserialize, Serialize};

use abstio::{CityName, MapName};
use abstutil::{deserialize_btreemap, serialize_btreemap, Tags, Timer};
use geom::{Distance, GPSBounds, PolyLine, Polygon, Pt2D};

use crate::make::initial::lane_specs::get_lane_specs_ltr;
use crate::{
    osm, Amenity, AreaType, Direction, DrivingSide, IntersectionType, LaneType, MapConfig,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct RawMap {
    pub name: MapName,
    #[serde(
        serialize_with = "serialize_btreemap",
        deserialize_with = "deserialize_btreemap"
    )]
    pub roads: BTreeMap<OriginalRoad, RawRoad>,
    #[serde(
        serialize_with = "serialize_btreemap",
        deserialize_with = "deserialize_btreemap"
    )]
    pub intersections: BTreeMap<osm::NodeID, RawIntersection>,
    #[serde(
        serialize_with = "serialize_btreemap",
        deserialize_with = "deserialize_btreemap"
    )]
    pub buildings: BTreeMap<osm::OsmID, RawBuilding>,
    pub areas: Vec<RawArea>,
    pub parking_lots: Vec<RawParkingLot>,
    pub parking_aisles: Vec<(osm::WayID, Vec<Pt2D>)>,

    pub boundary_polygon: Polygon,
    pub gps_bounds: GPSBounds,
    pub config: MapConfig,
}

/// A way to refer to roads across many maps and over time. Also trivial to relate with OSM to find
/// upstream problems.
//
// - Using LonLat is more indirect, and f64's need to be trimmed and compared carefully with epsilon
//   checks.
// - TODO Look at some stable ID standard like linear referencing
// (https://github.com/opentraffic/architecture/issues/1).
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OriginalRoad {
    pub osm_way_id: osm::WayID,
    pub i1: osm::NodeID,
    pub i2: osm::NodeID,
}

impl fmt::Display for OriginalRoad {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "OriginalRoad({} from {} to {}",
            self.osm_way_id, self.i1, self.i2
        )
    }
}
impl fmt::Debug for OriginalRoad {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self)
    }
}

impl OriginalRoad {
    pub fn new(way: i64, (i1, i2): (i64, i64)) -> OriginalRoad {
        OriginalRoad {
            osm_way_id: osm::WayID(way),
            i1: osm::NodeID(i1),
            i2: osm::NodeID(i2),
        }
    }

    /// Prints the OriginalRoad in a way that can be copied to Rust code.
    pub fn as_string_code(&self) -> String {
        format!(
            "OriginalRoad::new({}, ({}, {}))",
            self.osm_way_id.0, self.i1.0, self.i2.0
        )
    }

    // TODO Doesn't handle two roads between the same pair of intersections
    pub fn common_endpt(&self, other: OriginalRoad) -> osm::NodeID {
        #![allow(clippy::suspicious_operation_groupings)]
        if self.i1 == other.i1 || self.i1 == other.i2 {
            return self.i1;
        }
        if self.i2 == other.i1 || self.i2 == other.i2 {
            return self.i2;
        }
        panic!("{:?} and {:?} have no common_endpt", self, other);
    }
}

impl RawMap {
    pub fn blank(name: MapName) -> RawMap {
        RawMap {
            name,
            roads: BTreeMap::new(),
            intersections: BTreeMap::new(),
            buildings: BTreeMap::new(),
            areas: Vec::new(),
            parking_lots: Vec::new(),
            parking_aisles: Vec::new(),
            // Some nonsense thing
            boundary_polygon: Polygon::rectangle(1.0, 1.0),
            gps_bounds: GPSBounds::new(),
            config: MapConfig {
                driving_side: DrivingSide::Right,
                bikes_can_use_bus_lanes: true,
                inferred_sidewalks: true,
                street_parking_spot_length: Distance::meters(8.0),
                turn_on_red: true,
            },
        }
    }

    // TODO Might be better to maintain this instead of doing a search everytime.
    pub fn roads_per_intersection(&self, i: osm::NodeID) -> Vec<OriginalRoad> {
        let mut results = Vec::new();
        for id in self.roads.keys() {
            if id.i1 == i || id.i2 == i {
                results.push(*id);
            }
        }
        results
    }

    pub fn new_osm_node_id(&self, start: i64) -> osm::NodeID {
        assert!(start < 0);
        // Slow, but deterministic.
        let mut osm_node_id = start;
        loop {
            if self.intersections.keys().any(|i| i.0 == osm_node_id) {
                osm_node_id -= 1;
            } else {
                return osm::NodeID(osm_node_id);
            }
        }
    }

    // TODO Almost gone...
    pub fn new_osm_way_id(&self, start: i64) -> osm::WayID {
        assert!(start < 0);
        // Slow, but deterministic.
        let mut osm_way_id = start;
        loop {
            // TODO Only checks roads, doesn't handle collisions with buildings, areas, parking
            // lots
            if self.roads.keys().any(|r| r.osm_way_id.0 == osm_way_id) {
                osm_way_id -= 1;
            } else {
                return osm::WayID(osm_way_id);
            }
        }
    }

    /// (Intersection polygon, polygons for roads, list of labeled polygons to debug)
    #[allow(clippy::type_complexity)]
    pub fn preview_intersection(
        &self,
        id: osm::NodeID,
    ) -> Result<(Polygon, Vec<Polygon>, Vec<(String, Polygon)>)> {
        use crate::make::initial;

        let intersection_roads: BTreeSet<OriginalRoad> =
            self.roads_per_intersection(id).into_iter().collect();
        let mut roads = BTreeMap::new();
        for r in &intersection_roads {
            roads.insert(*r, initial::Road::new(*r, &self.roads[r], &self.config)?);
        }

        // trim_roads_for_merging will be empty unless we've called merge_short_road
        let (poly, debug) = initial::intersection_polygon(
            id,
            intersection_roads,
            &mut roads,
            &self.intersections[&id].trim_roads_for_merging,
        )?;
        Ok((
            poly,
            roads
                .values()
                .map(|r| r.trimmed_center_pts.make_polygons(2.0 * r.half_width))
                .collect(),
            debug,
        ))
    }

    /// Generate the trimmed `PolyLine` for a single RawRoad by calculating both intersections
    pub fn trimmed_road_geometry(&self, road: OriginalRoad) -> Option<PolyLine> {
        use crate::make::initial;

        let mut roads = BTreeMap::new();
        for id in [road.i1, road.i2] {
            for r in self.roads_per_intersection(id) {
                roads.insert(
                    r,
                    initial::Road::new(r, &self.roads[&r], &self.config).ok()?,
                );
            }
        }
        for id in [road.i1, road.i2] {
            initial::intersection_polygon(
                id,
                self.roads_per_intersection(id).into_iter().collect(),
                &mut roads,
                // TODO Not sure if we should use this or not
                &BTreeMap::new(),
            )
            .unwrap();
        }

        Some(roads.remove(&road).unwrap().trimmed_center_pts)
    }

    pub fn save(&self) {
        abstio::write_binary(abstio::path_raw_map(&self.name), self)
    }

    pub fn get_city_name(&self) -> &CityName {
        &self.name.city
    }
}

// Mutations and supporting queries
impl RawMap {
    pub fn can_delete_intersection(&self, i: osm::NodeID) -> bool {
        self.roads_per_intersection(i).is_empty()
    }

    pub fn delete_intersection(&mut self, id: osm::NodeID) {
        if !self.can_delete_intersection(id) {
            panic!(
                "Can't delete_intersection {}, must have roads connected",
                id
            );
        }
        self.intersections.remove(&id).unwrap();
    }

    pub fn move_intersection(&mut self, id: osm::NodeID, point: Pt2D) -> Option<Vec<OriginalRoad>> {
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

    pub fn closest_intersection(&self, pt: Pt2D) -> osm::NodeID {
        self.intersections
            .iter()
            .min_by_key(|(_, i)| i.point.dist_to(pt))
            .map(|(id, _)| *id)
            .unwrap()
    }

    pub fn path_dist_to(&self, from: osm::NodeID, to: osm::NodeID) -> Option<Distance> {
        let mut graph = DiGraphMap::new();
        for (id, r) in &self.roads {
            graph.add_edge(id.i1, id.i2, id);
            if !r.osm_tags.contains_key("oneway") {
                graph.add_edge(id.i2, id.i1, id);
            }
        }
        petgraph::algo::dijkstra(&graph, from, Some(to), |(_, _, r)| {
            // TODO Expensive!
            self.roads[r].length()
        })
        .get(&to)
        .cloned()
    }

    /// (the surviving intersection, the deleted intersection, deleted roads, new roads)
    pub fn merge_short_road(
        &mut self,
        short: OriginalRoad,
    ) -> Result<(
        osm::NodeID,
        osm::NodeID,
        Vec<OriginalRoad>,
        Vec<OriginalRoad>,
    )> {
        // If either intersection attached to this road has been deleted, then we're probably
        // dealing with a short segment in the middle of a cluster of intersections. Just delete
        // the segment and move on.
        if !self.intersections.contains_key(&short.i1)
            || !self.intersections.contains_key(&short.i2)
        {
            self.roads.remove(&short).unwrap();
            bail!(
                "One endpoint of {} has already been deleted, skipping",
                short
            );
        }

        // First a sanity check.
        {
            let i1 = &self.intersections[&short.i1];
            let i2 = &self.intersections[&short.i2];
            if i1.intersection_type == IntersectionType::Border
                || i2.intersection_type == IntersectionType::Border
            {
                bail!("{} touches a border", short);
            }
        }

        // TODO Fix up turn restrictions. Many cases:
        // [ ] road we're deleting has simple restrictions
        // [ ] road we're deleting has complicated restrictions
        // [X] road we're deleting is the target of a simple BanTurns restriction
        // [ ] road we're deleting is the target of a simple OnlyAllowTurns restriction
        // [ ] road we're deleting is the target of a complicated restriction
        // [X] road we're deleting is the 'via' of a complicated restriction
        // [ ] road we're deleting has turn lanes that wind up orphaning something

        let (i1, i2) = (short.i1, short.i2);
        if i1 == i2 {
            bail!("Can't merge {} -- it's a loop on {}", short, i1);
        }
        // Remember the original connections to i1 before we merge. None of these will change IDs.
        let mut connected_to_i1 = self.roads_per_intersection(i1);
        connected_to_i1.retain(|x| *x != short);

        // Retain some geometry...
        {
            let mut trim_roads_for_merging = BTreeMap::new();
            for i in [i1, i2] {
                for r in self.roads_per_intersection(i) {
                    // If we keep this in there, it might accidentally overwrite the
                    // trim_roads_for_merging key for a surviving road!
                    if r == short {
                        continue;
                    }
                    // If we're going to delete this later, don't bother!
                    // TODO When we do automatic consolidation and don't just look for this tag,
                    // we'll need to get more clever here, or temporarily apply this tag.
                    if self.roads[&r].osm_tags.is("junction", "intersection") {
                        continue;
                    }

                    if let Some(pl) = self.trimmed_road_geometry(r) {
                        if r.i1 == i {
                            if trim_roads_for_merging.contains_key(&(r.osm_way_id, true)) {
                                panic!("trim_roads_for_merging has an i1 duplicate for {}", r);
                            }
                            trim_roads_for_merging.insert((r.osm_way_id, true), pl.first_pt());
                        } else {
                            if trim_roads_for_merging.contains_key(&(r.osm_way_id, false)) {
                                panic!("trim_roads_for_merging has an i2 duplicate for {}", r);
                            }
                            trim_roads_for_merging.insert((r.osm_way_id, false), pl.last_pt());
                        }
                    } else {
                        panic!("No trimmed_road_geometry at all for {}", r);
                    }
                }
            }
            self.intersections
                .get_mut(&i1)
                .unwrap()
                .trim_roads_for_merging
                .extend(trim_roads_for_merging);
        }

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
        let mut old_to_new = BTreeMap::new();
        let mut new_to_old = BTreeMap::new();
        for r in self.roads_per_intersection(i2) {
            deleted.push(r);
            let road = self.roads.remove(&r).unwrap();
            let mut new_id = r;
            if r.i1 == i2 {
                new_id.i1 = i1;
            } else {
                assert_eq!(r.i2, i2);
                new_id.i2 = i1;
            }
            old_to_new.insert(r, new_id);
            new_to_old.insert(new_id, r);

            self.roads.insert(new_id, road);
            created.push(new_id);
        }

        // If we're deleting the target of a simple restriction somewhere, update it.
        for (from_id, road) in &mut self.roads {
            let mut fix_trs = Vec::new();
            for (rt, to) in road.turn_restrictions.drain(..) {
                if to == short && rt == RestrictionType::BanTurns {
                    // Remove this restriction, replace it with a new one to each of the successors
                    // of the deleted road. Depending if the intersection we kept is the one
                    // connecting these two roads, the successors differ.
                    if new_to_old
                        .get(from_id)
                        .cloned()
                        .unwrap_or(*from_id)
                        .common_endpt(short)
                        == i1
                    {
                        for x in &created {
                            fix_trs.push((rt, *x));
                        }
                    } else {
                        for x in &connected_to_i1 {
                            fix_trs.push((rt, *x));
                        }
                    }
                } else {
                    fix_trs.push((rt, to));
                }
            }
            road.turn_restrictions = fix_trs;
        }

        // If we're deleting the 'via' of a complicated restriction somewhere, change it to a
        // simple restriction.
        for road in self.roads.values_mut() {
            let mut add = Vec::new();
            road.complicated_turn_restrictions.retain(|(via, to)| {
                if *via == short {
                    // Depending which intersection we're deleting, the ID of 'to' might change
                    let to_id = old_to_new.get(to).cloned().unwrap_or(*to);
                    add.push((RestrictionType::BanTurns, to_id));
                    false
                } else {
                    true
                }
            });
            road.turn_restrictions.extend(add);
        }

        Ok((i1, i2, deleted, created))
    }

    /// Look for short roads that should be merged, and mark them as junction=intersection.
    pub fn auto_mark_junctions(&mut self) -> Vec<OriginalRoad> {
        let threshold = Distance::meters(20.0);

        // Simplest start: look for short roads connected to traffic signals.
        //
        // (This will miss sequences of short roads with stop signs in between a cluster of traffic
        // signals)
        //
        // After trying out around Loop 101, what we really want to do is find clumps of 2 or 4
        // traffic signals, find all the segments between them, and merge those.
        let mut results = Vec::new();
        for (id, road) in &self.roads {
            if road.osm_tags.is("junction", "intersection") {
                continue;
            }
            let i1 = self.intersections[&id.i1].intersection_type;
            let i2 = self.intersections[&id.i2].intersection_type;
            if i1 == IntersectionType::Border || i2 == IntersectionType::Border {
                continue;
            }
            if i1 != IntersectionType::TrafficSignal && i2 != IntersectionType::TrafficSignal {
                continue;
            }
            if let Ok((pl, _)) = road.get_geometry(*id, &self.config) {
                if pl.length() <= threshold {
                    results.push(*id);
                }
            }
        }

        for id in &results {
            self.roads
                .get_mut(id)
                .unwrap()
                .osm_tags
                .insert("junction", "intersection");
        }
        results
    }

    /// Run a sequence of transformations to the RawMap before converting it to a full Map.
    ///
    /// We don't want to run these during the OSM->RawMap import stage, because we want to use the
    /// map_editor tool to debug the RawMap.
    pub fn run_all_simplifications(
        &mut self,
        consolidate_all_intersections: bool,
        timer: &mut Timer,
    ) {
        timer.start("trimming dead-end cycleways (round 1)");
        crate::make::collapse_intersections::trim_deadends(self);
        timer.stop("trimming dead-end cycleways (round 1)");

        timer.start("snap separate cycleways");
        crate::make::snappy::snap_cycleways(self);
        timer.stop("snap separate cycleways");

        // More dead-ends can be created after snapping cycleways. But also, snapping can be easier
        // to do after trimming some dead-ends. So... just run it twice.
        timer.start("trimming dead-end cycleways (round 2)");
        crate::make::collapse_intersections::trim_deadends(self);
        timer.stop("trimming dead-end cycleways (round 2)");

        crate::make::remove_disconnected::remove_disconnected_roads(self, timer);

        timer.start("merging short roads");
        crate::make::merge_intersections::merge_short_roads(self, consolidate_all_intersections);
        timer.stop("merging short roads");

        timer.start("collapsing degenerate intersections");
        crate::make::collapse_intersections::collapse(self);
        timer.stop("collapsing degenerate intersections");
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RawRoad {
    /// This is effectively a PolyLine, except there's a case where we need to plumb forward
    /// cul-de-sac roads for roundabout handling. No transformation of these points whatsoever has
    /// happened.
    pub center_points: Vec<Pt2D>,
    pub osm_tags: Tags,
    pub turn_restrictions: Vec<(RestrictionType, OriginalRoad)>,
    /// (via, to). For turn restrictions where 'via' is an entire road. Only BanTurns.
    pub complicated_turn_restrictions: Vec<(OriginalRoad, OriginalRoad)>,
    pub percent_incline: f64,
    /// Is there a tagged crosswalk near each end of the road?
    pub crosswalk_forward: bool,
    pub crosswalk_backward: bool,
}

impl RawRoad {
    /// Returns the corrected center and total width
    pub fn get_geometry(&self, id: OriginalRoad, cfg: &MapConfig) -> Result<(PolyLine, Distance)> {
        let lane_specs = get_lane_specs_ltr(&self.osm_tags, cfg);
        let mut total_width = Distance::ZERO;
        let mut sidewalk_right = None;
        let mut sidewalk_left = None;
        for l in &lane_specs {
            total_width += l.width;
            if l.lt.is_walkable() {
                if l.dir == Direction::Back {
                    sidewalk_left = Some(l.width);
                } else {
                    sidewalk_right = Some(l.width);
                }
            }
        }

        // If there's a sidewalk on only one side, adjust the true center of the road.
        let mut true_center =
            PolyLine::new(self.center_points.clone()).with_context(|| id.to_string())?;
        match (sidewalk_right, sidewalk_left) {
            (Some(w), None) => {
                true_center = true_center.must_shift_right(w / 2.0);
            }
            (None, Some(w)) => {
                true_center = true_center.must_shift_right(w / 2.0);
            }
            _ => {}
        }

        Ok((true_center, total_width))
    }

    // TODO For the moment, treating all rail things as light rail
    pub fn is_light_rail(&self) -> bool {
        self.osm_tags.is_any("railway", vec!["light_rail", "rail"])
    }

    pub fn is_footway(&self) -> bool {
        self.osm_tags.is_any(
            osm::HIGHWAY,
            vec![
                "cycleway",
                "footway",
                "path",
                "pedestrian",
                "steps",
                "track",
            ],
        )
    }

    pub fn is_service(&self) -> bool {
        self.osm_tags.is(osm::HIGHWAY, "service")
    }

    pub fn is_cycleway(&self, cfg: &MapConfig) -> bool {
        // Don't repeat the logic looking at the tags, just see what lanes we'll create
        let mut bike = false;
        for spec in get_lane_specs_ltr(&self.osm_tags, cfg) {
            if spec.lt == LaneType::Biking {
                bike = true;
            } else if spec.lt != LaneType::Shoulder {
                return false;
            }
        }
        bike
    }

    pub fn length(&self) -> Distance {
        PolyLine::unchecked_new(self.center_points.clone()).length()
    }

    pub fn get_zorder(&self) -> isize {
        if let Some(layer) = self.osm_tags.get("layer") {
            match layer.parse::<f64>() {
                // Just drop .5 for now
                Ok(l) => l as isize,
                Err(_) => {
                    warn!(
                        "Weird layer={} on {:?}",
                        layer,
                        self.osm_tags.get(osm::OSM_WAY_ID)
                    );
                    0
                }
            }
        } else {
            0
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RawIntersection {
    /// Represents the original place where OSM center-lines meet. This may be meaningless beyond
    /// RawMap; roads and intersections get merged and deleted.
    pub point: Pt2D,
    pub intersection_type: IntersectionType,
    pub elevation: Distance,

    // true if src_i matches this intersection (or the deleted/consolidated one, whatever)
    pub trim_roads_for_merging: BTreeMap<(osm::WayID, bool), Pt2D>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RawBuilding {
    pub polygon: Polygon,
    pub osm_tags: Tags,
    pub public_garage_name: Option<String>,
    pub num_parking_spots: usize,
    pub amenities: Vec<Amenity>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RawArea {
    pub area_type: AreaType,
    pub polygon: Polygon,
    pub osm_tags: Tags,
    pub osm_id: osm::OsmID,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RawParkingLot {
    pub osm_id: osm::OsmID,
    pub polygon: Polygon,
    pub osm_tags: Tags,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RestrictionType {
    BanTurns,
    OnlyAllowTurns,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TurnRestriction(pub OriginalRoad, pub RestrictionType, pub OriginalRoad);

impl RestrictionType {
    pub fn new(restriction: &str) -> Option<RestrictionType> {
        // TODO There's a huge space of things not represented yet: time conditions, bus-only, no
        // right turn on red...

        // There are so many possibilities:
        // https://taginfo.openstreetmap.org/keys/restriction#values
        // Just attempt to bucket into allow / deny.
        if restriction.contains("no_") || restriction == "psv" {
            Some(RestrictionType::BanTurns)
        } else if restriction.contains("only_") {
            Some(RestrictionType::OnlyAllowTurns)
        } else {
            None
        }
    }
}
