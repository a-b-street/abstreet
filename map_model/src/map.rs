//! A bunch of (mostly read-only) queries on a Map.

use std::collections::{BTreeMap, BTreeSet, HashSet, VecDeque};

use anyhow::Result;
use petgraph::graphmap::UnGraphMap;
use serde::{Deserialize, Serialize};

use abstio::MapName;
use abstutil::Timer;
use geom::{Bounds, Distance, GPSBounds, Polygon, Pt2D, Ring, Time};

use crate::raw::{OriginalRoad, RawMap};
use crate::{
    osm, Area, AreaID, Building, BuildingID, BuildingType, BusRoute, BusRouteID, BusStop,
    BusStopID, ControlStopSign, ControlTrafficSignal, Intersection, IntersectionID, Lane, LaneID,
    LaneType, Map, MapEdits, MovementID, OffstreetParking, ParkingLot, ParkingLotID, Path,
    PathConstraints, PathRequest, Pathfinder, Position, Road, RoadID, Turn, TurnID, TurnType, Zone,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MapConfig {
    /// If true, driving happens on the right side of the road (USA). If false, on the left
    /// (Australia).
    pub driving_side: DrivingSide,
    pub bikes_can_use_bus_lanes: bool,
    /// If true, roads without explicitly tagged sidewalks may have sidewalks or shoulders. If
    /// false, no sidewalks will be inferred if not tagged in OSM, and separate sidewalks will be
    /// included.
    pub inferred_sidewalks: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq)]
pub enum DrivingSide {
    Right,
    Left,
}

impl Map {
    pub fn new(path: String, timer: &mut Timer) -> Map {
        if path.contains("/maps/") {
            match abstio::maybe_read_binary(path.clone(), timer) {
                Ok(map) => {
                    let mut map: Map = map;
                    map.edits = map.new_edits();

                    if false {
                        use abstutil::{prettyprint_usize, serialized_size_bytes};
                        info!(
                            "Total map size: {} bytes",
                            prettyprint_usize(serialized_size_bytes(&map))
                        );
                        info!(
                            "- {} roads: {} bytes",
                            prettyprint_usize(map.roads.len()),
                            prettyprint_usize(serialized_size_bytes(&map.roads))
                        );
                        info!(
                            "- {} lanes: {} bytes",
                            prettyprint_usize(map.lanes.len()),
                            prettyprint_usize(serialized_size_bytes(&map.lanes))
                        );
                        info!(
                            "- {} intersections: {} bytes",
                            prettyprint_usize(map.intersections.len()),
                            prettyprint_usize(serialized_size_bytes(&map.intersections))
                        );
                        info!(
                            "- {} turns: {} bytes",
                            prettyprint_usize(map.turns.len()),
                            prettyprint_usize(serialized_size_bytes(&map.turns))
                        );
                        info!(
                            "- {} buildings: {} bytes",
                            prettyprint_usize(map.buildings.len()),
                            prettyprint_usize(serialized_size_bytes(&map.buildings))
                        );
                        info!(
                            "- {} areas: {} bytes",
                            prettyprint_usize(map.areas.len()),
                            prettyprint_usize(serialized_size_bytes(&map.areas))
                        );
                        info!(
                            "- {} parking lots: {} bytes",
                            prettyprint_usize(map.parking_lots.len()),
                            prettyprint_usize(serialized_size_bytes(&map.parking_lots))
                        );
                        info!(
                            "- {} zones: {} bytes",
                            prettyprint_usize(map.zones.len()),
                            prettyprint_usize(serialized_size_bytes(&map.zones))
                        );
                        // This is the partridge in the pear tree, I suppose
                        info!(
                            "- pathfinder: {} bytes",
                            prettyprint_usize(serialized_size_bytes(&map.pathfinder))
                        );
                    }

                    return map;
                }
                Err(err) => {
                    error!("\nError loading {}: {}\n", path, err);
                    if err.to_string().contains("No such file") {
                        error!(
                            "{} is missing. You may need to do: cargo run --bin updater",
                            path
                        );
                    } else {
                        error!(
                            "{} is out-of-date. You may need to update your build (git pull) or \
                             download new data (cargo run --bin updater). If this is a custom \
                             map, you need to import it again.",
                            path
                        );
                    }
                    error!(
                        "Check https://dabreegster.github.io/abstreet/dev/index.html and file an \
                         issue if you have trouble."
                    );

                    std::process::exit(1);
                }
            }
        }

        let raw: RawMap = abstio::read_binary(path, timer);
        Map::create_from_raw(raw, true, false, timer)
    }

    /// If you have to deserialize a `Map` directly, call this after. Prefer using `Map::new`
    /// though.
    pub fn map_loaded_directly(&mut self) {
        self.edits = self.new_edits();
    }

    /// Just for temporary std::mem::replace tricks.
    pub fn blank() -> Map {
        Map {
            roads: Vec::new(),
            lanes: Vec::new(),
            intersections: Vec::new(),
            turns: BTreeMap::new(),
            buildings: Vec::new(),
            bus_stops: BTreeMap::new(),
            bus_routes: Vec::new(),
            areas: Vec::new(),
            parking_lots: Vec::new(),
            zones: Vec::new(),
            boundary_polygon: Ring::must_new(vec![
                Pt2D::new(0.0, 0.0),
                Pt2D::new(1.0, 0.0),
                Pt2D::new(1.0, 1.0),
                Pt2D::new(0.0, 0.0),
            ])
            .to_polygon(),
            stop_signs: BTreeMap::new(),
            traffic_signals: BTreeMap::new(),
            gps_bounds: GPSBounds::new(),
            bounds: Bounds::new(),
            config: MapConfig {
                driving_side: DrivingSide::Right,
                bikes_can_use_bus_lanes: true,
                inferred_sidewalks: true,
            },
            pathfinder: Pathfinder::Dijkstra,
            pathfinder_dirty: false,
            name: MapName {
                city: "blank city".to_string(),
                map: "blank".to_string(),
            },
            edits: MapEdits::new(),
        }
    }

    pub fn all_roads(&self) -> &Vec<Road> {
        &self.roads
    }

    pub fn all_lanes(&self) -> &Vec<Lane> {
        &self.lanes
    }

    pub fn all_intersections(&self) -> &Vec<Intersection> {
        &self.intersections
    }

    pub fn all_turns(&self) -> &BTreeMap<TurnID, Turn> {
        &self.turns
    }

    pub fn all_buildings(&self) -> &Vec<Building> {
        &self.buildings
    }

    pub fn all_areas(&self) -> &Vec<Area> {
        &self.areas
    }

    pub fn all_parking_lots(&self) -> &Vec<ParkingLot> {
        &self.parking_lots
    }

    pub fn all_zones(&self) -> &Vec<Zone> {
        &self.zones
    }

    pub fn maybe_get_r(&self, id: RoadID) -> Option<&Road> {
        self.roads.get(id.0)
    }

    pub fn maybe_get_l(&self, id: LaneID) -> Option<&Lane> {
        self.lanes.get(id.0)
    }

    pub fn maybe_get_i(&self, id: IntersectionID) -> Option<&Intersection> {
        self.intersections.get(id.0)
    }

    pub fn maybe_get_t(&self, id: TurnID) -> Option<&Turn> {
        self.turns.get(&id)
    }

    pub fn maybe_get_b(&self, id: BuildingID) -> Option<&Building> {
        self.buildings.get(id.0)
    }

    pub fn maybe_get_pl(&self, id: ParkingLotID) -> Option<&ParkingLot> {
        self.parking_lots.get(id.0)
    }

    pub fn maybe_get_a(&self, id: AreaID) -> Option<&Area> {
        self.areas.get(id.0)
    }

    pub fn maybe_get_bs(&self, id: BusStopID) -> Option<&BusStop> {
        self.bus_stops.get(&id)
    }

    pub fn maybe_get_stop_sign(&self, id: IntersectionID) -> Option<&ControlStopSign> {
        self.stop_signs.get(&id)
    }

    pub fn maybe_get_traffic_signal(&self, id: IntersectionID) -> Option<&ControlTrafficSignal> {
        self.traffic_signals.get(&id)
    }

    pub fn maybe_get_br(&self, route: BusRouteID) -> Option<&BusRoute> {
        self.bus_routes.get(route.0)
    }

    pub fn get_r(&self, id: RoadID) -> &Road {
        &self.roads[id.0]
    }

    pub fn get_l(&self, id: LaneID) -> &Lane {
        &self.lanes[id.0]
    }

    pub fn get_i(&self, id: IntersectionID) -> &Intersection {
        &self.intersections[id.0]
    }

    pub fn get_t(&self, id: TurnID) -> &Turn {
        // When pathfinding breaks, seeing this TurnID is useful.
        if let Some(ref t) = self.turns.get(&id) {
            t
        } else {
            panic!("Can't get_t({})", id);
        }
    }

    pub fn get_b(&self, id: BuildingID) -> &Building {
        &self.buildings[id.0]
    }

    pub fn get_a(&self, id: AreaID) -> &Area {
        &self.areas[id.0]
    }

    pub fn get_pl(&self, id: ParkingLotID) -> &ParkingLot {
        &self.parking_lots[id.0]
    }

    pub fn get_stop_sign(&self, id: IntersectionID) -> &ControlStopSign {
        &self.stop_signs[&id]
    }

    pub fn get_traffic_signal(&self, id: IntersectionID) -> &ControlTrafficSignal {
        &self.traffic_signals[&id]
    }

    // All these helpers should take IDs and return objects.

    pub fn get_turns_in_intersection(&self, id: IntersectionID) -> Vec<&Turn> {
        self.get_i(id)
            .turns
            .iter()
            .map(|t| self.get_t(*t))
            .collect()
    }

    /// The turns may belong to two different intersections!
    pub fn get_turns_from_lane(&self, l: LaneID) -> Vec<&Turn> {
        let lane = self.get_l(l);
        let mut turns: Vec<&Turn> = self
            .get_i(lane.dst_i)
            .turns
            .iter()
            .map(|t| self.get_t(*t))
            .filter(|t| t.id.src == l)
            .collect();
        // Sidewalks/shoulders are bidirectional
        if lane.is_walkable() {
            for t in &self.get_i(lane.src_i).turns {
                if t.src == l {
                    turns.push(self.get_t(*t));
                }
            }
        }
        turns
    }

    pub fn get_turns_to_lane(&self, l: LaneID) -> Vec<&Turn> {
        let lane = self.get_l(l);
        let mut turns: Vec<&Turn> = self
            .get_i(lane.src_i)
            .turns
            .iter()
            .map(|t| self.get_t(*t))
            .filter(|t| t.id.dst == l)
            .collect();
        // Sidewalks/shoulders are bidirectional
        if lane.is_walkable() {
            for t in &self.get_i(lane.dst_i).turns {
                if t.dst == l {
                    turns.push(self.get_t(*t));
                }
            }
        }
        turns
    }

    pub fn get_turn_between(
        &self,
        from: LaneID,
        to: LaneID,
        parent: IntersectionID,
    ) -> Option<TurnID> {
        self.get_i(parent)
            .turns
            .iter()
            .find(|t| t.src == from && t.dst == to)
            .cloned()
    }

    pub fn get_next_turns_and_lanes(
        &self,
        from: LaneID,
        parent: IntersectionID,
    ) -> Vec<(&Turn, &Lane)> {
        self.get_i(parent)
            .turns
            .iter()
            .filter(|t| t.src == from)
            .map(|t| (self.get_t(*t), self.get_l(t.dst)))
            .collect()
    }

    pub fn get_turns_for(&self, from: LaneID, constraints: PathConstraints) -> Vec<&Turn> {
        let mut turns: Vec<&Turn> = self
            .get_next_turns_and_lanes(from, self.get_l(from).dst_i)
            .into_iter()
            .filter(|(_, l)| constraints.can_use(l, self))
            .map(|(t, _)| t)
            .collect();
        // Sidewalks are bidirectional
        if constraints == PathConstraints::Pedestrian {
            turns.extend(
                self.get_next_turns_and_lanes(from, self.get_l(from).src_i)
                    .into_iter()
                    .filter(|(_, l)| constraints.can_use(l, self))
                    .map(|(t, _)| t),
            );
        }
        turns
    }

    pub fn get_next_roads(&self, from: RoadID) -> BTreeSet<RoadID> {
        let mut roads: BTreeSet<RoadID> = BTreeSet::new();
        let r = self.get_r(from);
        for id in vec![r.src_i, r.dst_i].into_iter() {
            roads.extend(self.get_i(id).roads.clone());
        }
        roads
    }

    pub fn get_parent(&self, id: LaneID) -> &Road {
        let l = self.get_l(id);
        self.get_r(l.parent)
    }

    pub fn get_gps_bounds(&self) -> &GPSBounds {
        &self.gps_bounds
    }

    pub fn get_bounds(&self) -> &Bounds {
        &self.bounds
    }

    pub fn get_city_name(&self) -> &String {
        &self.name.city
    }

    pub fn get_name(&self) -> &MapName {
        &self.name
    }

    pub fn all_bus_stops(&self) -> &BTreeMap<BusStopID, BusStop> {
        &self.bus_stops
    }

    pub fn get_bs(&self, stop: BusStopID) -> &BusStop {
        &self.bus_stops[&stop]
    }

    pub fn get_br(&self, route: BusRouteID) -> &BusRoute {
        &self.bus_routes[route.0]
    }

    pub fn all_bus_routes(&self) -> &Vec<BusRoute> {
        &self.bus_routes
    }

    pub fn get_bus_route(&self, name: &str) -> Option<&BusRoute> {
        self.bus_routes.iter().find(|r| r.full_name == name)
    }

    pub fn get_routes_serving_stop(&self, stop: BusStopID) -> Vec<&BusRoute> {
        let mut routes = Vec::new();
        for r in &self.bus_routes {
            if r.stops.contains(&stop) {
                routes.push(r);
            }
        }
        routes
    }

    pub fn building_to_road(&self, id: BuildingID) -> &Road {
        self.get_parent(self.get_b(id).sidewalk())
    }

    /// This and all_outgoing_borders are expensive to constantly repeat
    pub fn all_incoming_borders(&self) -> Vec<&Intersection> {
        let mut result: Vec<&Intersection> = Vec::new();
        for i in &self.intersections {
            if i.is_incoming_border() {
                result.push(i);
            }
        }
        result
    }

    pub fn all_outgoing_borders(&self) -> Vec<&Intersection> {
        let mut result: Vec<&Intersection> = Vec::new();
        for i in &self.intersections {
            if i.is_outgoing_border() {
                result.push(i);
            }
        }
        result
    }

    pub fn save(&self) {
        assert!(self.edits.edits_name.starts_with("Untitled Proposal"));
        assert!(self.edits.commands.is_empty());
        assert!(!self.pathfinder_dirty);
        abstio::write_binary(self.name.path(), self);
    }

    /// Cars trying to park near this building should head for the driving lane returned here, then
    /// start their search. Some parking lanes are connected to driving lanes that're "parking
    /// blackholes" -- if there are no free spots on that lane, then the roads force cars to a
    /// border.
    // TODO Making driving_connection do this.
    pub fn find_driving_lane_near_building(&self, b: BuildingID) -> LaneID {
        let sidewalk = self.get_b(b).sidewalk();
        if let Some(l) = self.get_parent(sidewalk).find_closest_lane(
            sidewalk,
            |l| PathConstraints::Car.can_use(l, self),
            self,
        ) {
            if !self.get_l(l).driving_blackhole {
                return l;
            }
        }

        let mut roads_queue: VecDeque<RoadID> = VecDeque::new();
        let mut visited: HashSet<RoadID> = HashSet::new();
        {
            let start = self.building_to_road(b).id;
            roads_queue.push_back(start);
            visited.insert(start);
        }

        loop {
            if roads_queue.is_empty() {
                panic!(
                    "Giving up looking for a driving lane near {}, searched {} roads: {:?}",
                    b,
                    visited.len(),
                    visited
                );
            }
            let r = self.get_r(roads_queue.pop_front().unwrap());

            for (l, lt) in r
                .children_forwards()
                .into_iter()
                .chain(r.children_backwards().into_iter())
            {
                if lt == LaneType::Driving {
                    if !self.get_l(l).driving_blackhole {
                        return l;
                    }
                }
            }

            for next_r in self.get_next_roads(r.id).into_iter() {
                if !visited.contains(&next_r) {
                    roads_queue.push_back(next_r);
                    visited.insert(next_r);
                }
            }
        }
    }

    pub fn get_boundary_polygon(&self) -> &Polygon {
        &self.boundary_polygon
    }

    pub fn pathfind(&self, req: PathRequest) -> Result<Path> {
        assert!(!self.pathfinder_dirty);
        self.pathfinder
            .pathfind(req.clone(), self)
            .ok_or_else(|| anyhow!("can't fulfill {}", req))
    }
    pub fn pathfind_avoiding_lanes(
        &self,
        req: PathRequest,
        avoid: BTreeSet<LaneID>,
    ) -> Option<Path> {
        assert!(!self.pathfinder_dirty);
        self.pathfinder.pathfind_avoiding_lanes(req, avoid, self)
    }

    pub fn should_use_transit(
        &self,
        start: Position,
        end: Position,
    ) -> Option<(BusStopID, Option<BusStopID>, BusRouteID)> {
        self.pathfinder.should_use_transit(self, start, end)
    }

    // None for SharedSidewalkCorners
    pub fn get_movement(&self, t: TurnID) -> Option<MovementID> {
        if let Some(ref ts) = self.maybe_get_traffic_signal(t.parent) {
            if self.get_t(t).turn_type == TurnType::SharedSidewalkCorner {
                return None;
            }
            for m in ts.movements.values() {
                if m.members.contains(&t) {
                    return Some(m.id);
                }
            }
            panic!("{} doesn't belong to any movements", t);
        }
        None
    }

    pub fn find_r_by_osm_id(&self, id: OriginalRoad) -> Result<RoadID> {
        for r in self.all_roads() {
            if r.orig_id == id {
                return Ok(r.id);
            }
        }
        bail!("Can't find {}", id)
    }

    pub fn find_i_by_osm_id(&self, id: osm::NodeID) -> Result<IntersectionID> {
        for i in self.all_intersections() {
            if i.orig_id == id {
                return Ok(i.id);
            }
        }
        bail!("Can't find {}", id)
    }

    pub fn find_b_by_osm_id(&self, id: osm::OsmID) -> Option<BuildingID> {
        for b in self.all_buildings() {
            if b.orig_id == id {
                return Some(b.id);
            }
        }
        None
    }

    pub fn find_br(&self, id: osm::RelationID) -> Option<BusRouteID> {
        for br in self.all_bus_routes() {
            if br.osm_rel_id == id {
                return Some(br.id);
            }
        }
        None
    }

    // TODO Sort of a temporary hack
    pub fn hack_override_offstreet_spots(&mut self, spots_per_bldg: usize) {
        for b in &mut self.buildings {
            if let OffstreetParking::Private(ref mut num_spots, _) = b.parking {
                *num_spots = spots_per_bldg;
            }
        }
    }
    pub fn hack_override_offstreet_spots_individ(&mut self, b: BuildingID, spots: usize) {
        let b = &mut self.buildings[b.0];
        if let OffstreetParking::Private(ref mut num_spots, _) = b.parking {
            *num_spots = spots;
        }
    }

    pub fn hack_override_bldg_type(&mut self, b: BuildingID, bldg_type: BuildingType) {
        self.buildings[b.0].bldg_type = bldg_type;
    }

    pub fn hack_override_orig_spawn_times(&mut self, br: BusRouteID, times: Vec<Time>) {
        self.bus_routes[br.0].orig_spawn_times = times.clone();
        self.bus_routes[br.0].spawn_times = times;
    }

    pub fn get_languages(&self) -> BTreeSet<&str> {
        let mut languages = BTreeSet::new();
        for r in self.all_roads() {
            for key in r.osm_tags.inner().keys() {
                if let Some(x) = key.strip_prefix("name:") {
                    languages.insert(x);
                }
            }
        }
        for b in self.all_buildings() {
            for a in &b.amenities {
                for key in a.names.0.keys() {
                    if let Some(lang) = key {
                        languages.insert(lang);
                    }
                }
            }
        }
        languages
    }

    pub fn get_config(&self) -> &MapConfig {
        &self.config
    }

    /// Simple search along undirected roads
    pub fn simple_path_btwn(&self, i1: IntersectionID, i2: IntersectionID) -> Option<Vec<RoadID>> {
        let mut graph: UnGraphMap<IntersectionID, RoadID> = UnGraphMap::new();
        for r in self.all_roads() {
            if !r.is_light_rail() {
                graph.add_edge(r.src_i, r.dst_i, r.id);
            }
        }
        let (_, path) = petgraph::algo::astar(
            &graph,
            i1,
            |i| i == i2,
            |(_, _, r)| self.get_r(*r).center_pts.length(),
            |_| Distance::ZERO,
        )?;
        Some(
            path.windows(2)
                .map(|pair| *graph.edge_weight(pair[0], pair[1]).unwrap())
                .collect(),
        )
    }
}
