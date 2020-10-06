// The convert_osm crate produces a RawMap from OSM and other data. Storing this intermediate
// structure is useful to iterate quickly on parts of the map importing pipeline without having to
// constantly read .osm files, and to visualize the intermediate state with map_editor.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use petgraph::graphmap::DiGraphMap;
use serde::{Deserialize, Serialize};

use abstutil::{deserialize_btreemap, serialize_btreemap, Tags, Timer};
use geom::{Circle, Distance, GPSBounds, PolyLine, Polygon, Pt2D};

use crate::make::initial::lane_specs::get_lane_specs_ltr;
use crate::{
    osm, AreaType, Direction, DrivingSide, IntersectionType, LaneType, MapConfig, NamePerLanguage,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct RawMap {
    pub city_name: String,
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
    pub intersections: BTreeMap<osm::NodeID, RawIntersection>,
    #[serde(
        serialize_with = "serialize_btreemap",
        deserialize_with = "deserialize_btreemap"
    )]
    pub buildings: BTreeMap<osm::OsmID, RawBuilding>,
    pub bus_routes: Vec<RawBusRoute>,
    pub areas: Vec<RawArea>,
    pub parking_lots: Vec<RawParkingLot>,
    pub parking_aisles: Vec<(osm::WayID, Vec<Pt2D>)>,

    pub boundary_polygon: Polygon,
    pub gps_bounds: GPSBounds,
    pub config: MapConfig,
}

// A way to refer to roads across many maps and over time. Also trivial to relate with OSM to find
// upstream problems.
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
}

impl RawMap {
    pub fn blank(city_name: &str, name: &str) -> RawMap {
        RawMap {
            city_name: city_name.to_string(),
            name: name.to_string(),
            roads: BTreeMap::new(),
            intersections: BTreeMap::new(),
            buildings: BTreeMap::new(),
            bus_routes: Vec::new(),
            areas: Vec::new(),
            parking_lots: Vec::new(),
            parking_aisles: Vec::new(),
            // Some nonsense thing
            boundary_polygon: Polygon::rectangle(1.0, 1.0),
            gps_bounds: GPSBounds::new(),
            config: MapConfig {
                driving_side: DrivingSide::Right,
                bikes_can_use_bus_lanes: true,
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

    // (Intersection polygon, polygons for roads, list of labeled polylines to debug)
    pub fn preview_intersection(
        &self,
        id: osm::NodeID,
        timer: &mut Timer,
    ) -> (Polygon, Vec<Polygon>, Vec<(String, Polygon)>) {
        use crate::make::initial;

        let i = initial::Intersection {
            id,
            polygon: Circle::new(Pt2D::new(0.0, 0.0), Distance::meters(1.0)).to_polygon(),
            roads: self.roads_per_intersection(id).into_iter().collect(),
            intersection_type: self.intersections[&id].intersection_type,
            elevation: self.intersections[&id].elevation,
        };
        let mut roads = BTreeMap::new();
        for r in &i.roads {
            roads.insert(
                *r,
                initial::Road::new(*r, &self.roads[r], self.config.driving_side),
            );
        }

        let (poly, debug) = initial::intersection_polygon(&i, &mut roads, timer).unwrap();
        (
            poly,
            roads
                .values()
                .map(|r| r.trimmed_center_pts.make_polygons(2.0 * r.half_width))
                .collect(),
            debug,
        )
    }

    pub fn save(&self) {
        abstutil::write_binary(
            abstutil::path(format!("input/raw_maps/{}.bin", self.name)),
            self,
        )
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
            PolyLine::unchecked_new(self.roads[r].center_points.clone()).length()
        })
        .get(&to)
        .cloned()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RawRoad {
    // This is effectively a PolyLine, except there's a case where we need to plumb forward
    // cul-de-sac roads for roundabout handling. No transformation of these points whatsoever has
    // happened.
    pub center_points: Vec<Pt2D>,
    pub osm_tags: Tags,
    pub turn_restrictions: Vec<(RestrictionType, OriginalRoad)>,
    // (via, to). For turn restrictions where 'via' is an entire road. Only BanTurns.
    pub complicated_turn_restrictions: Vec<(OriginalRoad, OriginalRoad)>,
}

impl RawRoad {
    // Returns the corrected center and total width
    pub fn get_geometry(
        &self,
        id: OriginalRoad,
        driving_side: DrivingSide,
    ) -> (PolyLine, Distance) {
        let lane_specs = get_lane_specs_ltr(&self.osm_tags, driving_side);
        let mut total_width = Distance::ZERO;
        let mut sidewalk_right = None;
        let mut sidewalk_left = None;
        for l in &lane_specs {
            total_width += l.width;
            if l.lt == LaneType::Sidewalk || l.lt == LaneType::Shoulder {
                if l.dir == Direction::Back {
                    sidewalk_left = Some(l.width);
                } else {
                    sidewalk_right = Some(l.width);
                }
            }
        }

        // If there's a sidewalk on only one side, adjust the true center of the road.
        let mut true_center = PolyLine::new(self.center_points.clone()).expect(&id.to_string());
        match (sidewalk_right, sidewalk_left) {
            (Some(w), None) => {
                true_center = true_center.must_shift_right(w / 2.0);
            }
            (None, Some(w)) => {
                true_center = true_center.must_shift_right(w / 2.0);
            }
            _ => {}
        }

        (true_center, total_width)
    }

    // TODO For the moment, treating all rail things as light rail
    pub fn is_light_rail(&self) -> bool {
        self.osm_tags.is_any("railway", vec!["light_rail", "rail"])
    }

    pub fn is_footway(&self) -> bool {
        self.osm_tags.is(osm::HIGHWAY, "pedestrian")
    }

    pub fn is_service(&self) -> bool {
        self.osm_tags.is(osm::HIGHWAY, "service")
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
    pub osm_tags: Tags,
    pub public_garage_name: Option<String>,
    pub num_parking_spots: usize,
    // (Name, amenity type)
    pub amenities: BTreeSet<(NamePerLanguage, String)>,
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

#[derive(Debug, Serialize, Deserialize)]
pub struct RawBusRoute {
    pub full_name: String,
    pub short_name: String,
    pub osm_rel_id: osm::RelationID,
    pub gtfs_trip_marker: Option<String>,
    // If not, light rail
    pub is_bus: bool,
    pub stops: Vec<RawBusStop>,
    pub border_start: Option<osm::NodeID>,
    pub border_end: Option<osm::NodeID>,
    // This is guaranteed to be in order and contiguous.
    pub all_pts: Vec<(osm::NodeID, Pt2D)>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RawBusStop {
    pub name: String,
    // Probably not an intersection, but this type is more convenient.
    pub vehicle_pos: (osm::NodeID, Pt2D),
    // Guaranteed to be filled out when RawMap is fully written.
    pub matched_road: Option<(OriginalRoad, Direction)>,
    // If it's not explicitly mapped, we'll do equiv_pos.
    pub ped_pos: Option<Pt2D>,
}
