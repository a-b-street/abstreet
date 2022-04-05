//! The convert_osm crate produces a RawMap from OSM and other data. Storing this intermediate
//! structure is useful to iterate quickly on parts of the map importing pipeline without having to
//! constantly read .osm files, and to visualize the intermediate state with map_editor.

#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate log;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use anyhow::{Context, Result};
use petgraph::graphmap::DiGraphMap;
use serde::{Deserialize, Serialize};

use abstio::{CityName, MapName};
use abstutil::{deserialize_btreemap, serialize_btreemap, Tags};
use geom::{Angle, Distance, GPSBounds, PolyLine, Polygon, Pt2D};

pub use self::geometry::intersection_polygon;
pub use self::lane_specs::get_lane_specs_ltr;
pub use self::types::{
    Amenity, AmenityType, AreaType, BufferType, Direction, DrivingSide, IntersectionType, LaneSpec,
    LaneType, MapConfig, NamePerLanguage, NORMAL_LANE_THICKNESS, SIDEWALK_THICKNESS,
};

pub mod geometry;
pub mod initial;
mod lane_specs;
pub mod osm;
mod transform;
mod types;

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
    pub transit_routes: Vec<RawTransitRoute>,
    #[serde(
        serialize_with = "serialize_btreemap",
        deserialize_with = "deserialize_btreemap"
    )]
    pub transit_stops: BTreeMap<String, RawTransitStop>,

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

    pub fn has_common_endpoint(&self, other: OriginalRoad) -> bool {
        if self.i1 == other.i1 || self.i1 == other.i2 {
            return true;
        }
        if self.i2 == other.i1 || self.i2 == other.i2 {
            return true;
        }
        false
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
            transit_routes: Vec::new(),
            transit_stops: BTreeMap::new(),
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
            let candidate = osm::WayID(osm_way_id);
            // TODO Doesn't handle collisions with areas or parking lots
            if self.roads.keys().any(|r| r.osm_way_id.0 == osm_way_id)
                || self
                    .buildings
                    .keys()
                    .any(|b| b == &osm::OsmID::Way(candidate))
            {
                osm_way_id -= 1;
            } else {
                return candidate;
            }
        }
    }

    /// (Intersection polygon, polygons for roads, list of labeled polygons to debug)
    #[allow(clippy::type_complexity)]
    pub fn preview_intersection(
        &self,
        id: osm::NodeID,
    ) -> Result<(Polygon, Vec<Polygon>, Vec<(String, Polygon)>)> {
        let intersection_roads: BTreeSet<OriginalRoad> =
            self.roads_per_intersection(id).into_iter().collect();
        let mut roads = BTreeMap::new();
        for r in &intersection_roads {
            roads.insert(*r, initial::Road::new(self, *r)?);
        }

        // trim_roads_for_merging will be empty unless we've called merge_short_road
        let (poly, debug) = intersection_polygon(
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
    pub fn trimmed_road_geometry(&self, road: OriginalRoad) -> Result<PolyLine> {
        let mut roads = BTreeMap::new();
        for id in [road.i1, road.i2] {
            for r in self.roads_per_intersection(id) {
                roads.insert(
                    r,
                    initial::Road::new(self, r).with_context(|| road.to_string())?,
                );
            }
        }
        for id in [road.i1, road.i2] {
            intersection_polygon(
                id,
                self.roads_per_intersection(id).into_iter().collect(),
                &mut roads,
                // TODO Not sure if we should use this or not
                &BTreeMap::new(),
            )
            .with_context(|| road.to_string())?;
        }

        Ok(roads.remove(&road).unwrap().trimmed_center_pts)
    }

    /// Returns the corrected (but untrimmed) center and total width for a road
    pub fn untrimmed_road_geometry(&self, id: OriginalRoad) -> Result<(PolyLine, Distance)> {
        let road = &self.roads[&id];
        let lane_specs = get_lane_specs_ltr(&road.osm_tags, &self.config);
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
            PolyLine::new(road.center_points.clone()).with_context(|| id.to_string())?;
        match (sidewalk_right, sidewalk_left) {
            (Some(w), None) => {
                true_center = true_center.must_shift_right(w / 2.0);
            }
            (None, Some(w)) => {
                true_center = true_center.must_shift_right(w / 2.0);
            }
            _ => {}
        }

        Ok((true_center, road.scale_width * total_width))
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
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RawRoad {
    /// This is effectively a PolyLine, except there's a case where we need to plumb forward
    /// cul-de-sac roads for roundabout handling. No transformation of these points whatsoever has
    /// happened.
    pub center_points: Vec<Pt2D>,
    /// Multiply the width of each lane by this ratio, to prevent overlapping roads.
    pub scale_width: f64,
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

    pub fn is_driveable(&self, cfg: &MapConfig) -> bool {
        get_lane_specs_ltr(&self.osm_tags, cfg)
            .into_iter()
            .any(|spec| spec.lt == LaneType::Driving)
    }

    pub fn is_oneway(&self) -> bool {
        self.osm_tags.is("oneway", "yes")
    }

    /// Points from first to last point. Undefined for loops.
    pub fn angle(&self) -> Angle {
        self.center_points[0].angle_to(*self.center_points.last().unwrap())
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

impl RawIntersection {
    fn is_border(&self) -> bool {
        self.intersection_type == IntersectionType::Border
    }
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RawTransitRoute {
    pub long_name: String,
    pub short_name: String,
    pub gtfs_id: String,
    /// This may begin and/or end inside or outside the map boundary.
    pub shape: PolyLine,
    /// Entries into transit_stops
    pub stops: Vec<String>,
    pub route_type: RawTransitType,
    // TODO Schedule
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum RawTransitType {
    Bus,
    Train,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RawTransitStop {
    pub gtfs_id: String,
    /// Only stops within a map's boundary are kept
    pub position: Pt2D,
    pub name: String,
}
