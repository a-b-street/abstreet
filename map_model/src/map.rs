//! A bunch of (mostly read-only) queries on a Map.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};

use anyhow::Result;
use petgraph::graphmap::{DiGraphMap, UnGraphMap};

use abstio::{CityName, MapName};
use abstutil::{prettyprint_usize, serialized_size_bytes, MultiMap, Tags, Timer};
use geom::{Bounds, Distance, Duration, GPSBounds, LonLat, PolyLine, Polygon, Pt2D, Ring, Time};
use raw_map::{RawBuilding, RawMap};

use crate::{
    osm, AmenityType, Area, AreaID, AreaType, Building, BuildingID, BuildingType, CommonEndpoint,
    CompressedMovementID, ControlStopSign, ControlTrafficSignal, DirectedRoadID, Direction,
    DrivingSide, Intersection, IntersectionControl, IntersectionID, IntersectionKind, Lane, LaneID,
    LaneType, Map, MapConfig, MapEdits, Movement, MovementID, OffstreetParking, OriginalRoad,
    ParkingLot, ParkingLotID, Path, PathConstraints, PathRequest, PathV2, Pathfinder,
    PathfinderCaching, Position, Road, RoadID, RoutingParams, TransitRoute, TransitRouteID,
    TransitStop, TransitStopID, Turn, TurnID, TurnType, Zone,
};

impl Map {
    /// Load a map from a local serialized Map or RawMap. Note this won't work on web. This should
    /// only be used by non-UI tools.
    pub fn load_synchronously(path: String, timer: &mut Timer) -> Map {
        if path.contains("/maps/") {
            match abstio::maybe_read_binary(path.clone(), timer) {
                Ok(map) => {
                    let mut map: Map = map;
                    map.map_loaded_directly(timer);
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
                        "Check https://a-b-street.github.io/docs/tech/dev/index.html and file an issue \
                         if you have trouble."
                    );

                    std::process::exit(1);
                }
            }
        }

        let raw: RawMap = abstio::read_binary(path, timer);
        Map::create_from_raw(raw, crate::RawToMapOptions::default(), timer)
    }

    /// After deserializing a map directly, call this after.
    pub fn map_loaded_directly(&mut self, timer: &mut Timer) {
        #![allow(clippy::logic_bug)]
        // For debugging map file sizes

        self.edits = self.new_edits();
        self.recalculate_road_to_buildings();
        self.recalculate_all_movements(timer);

        // Enable to work on shrinking map file sizes. Never run this on the web though --
        // trying to serialize fast_paths in wasm melts the browser, because the usize<->u32
        // translation there isn't meant to run on wasm.
        if cfg!(not(target_arch = "wasm32")) && false {
            let mut costs = vec![
                (
                    "roads",
                    self.roads.len(),
                    serialized_size_bytes(&self.roads),
                ),
                (
                    "intersections",
                    self.intersections.len(),
                    serialized_size_bytes(&self.intersections),
                ),
                (
                    "buildings",
                    self.buildings.len(),
                    serialized_size_bytes(&self.buildings),
                ),
                (
                    "areas",
                    self.areas.len(),
                    serialized_size_bytes(&self.areas),
                ),
                (
                    "parking lots",
                    self.parking_lots.len(),
                    serialized_size_bytes(&self.parking_lots),
                ),
                (
                    "zones",
                    self.zones.len(),
                    serialized_size_bytes(&self.zones),
                ),
                ("pathfinder", 1, serialized_size_bytes(&self.pathfinder)),
            ];
            costs.sort_by_key(|(_, _, bytes)| *bytes);
            costs.reverse();

            info!(
                "Total map size: {} bytes",
                prettyprint_usize(serialized_size_bytes(self))
            );
            for (name, number, bytes) in costs {
                info!(
                    "- {} {}: {} bytes",
                    prettyprint_usize(number),
                    name,
                    prettyprint_usize(bytes)
                );
            }
        }
    }

    /// Just for temporary std::mem::replace tricks.
    pub fn blank() -> Map {
        Map {
            roads: Vec::new(),
            intersections: Vec::new(),
            buildings: Vec::new(),
            transit_stops: BTreeMap::new(),
            transit_routes: Vec::new(),
            areas: Vec::new(),
            parking_lots: Vec::new(),
            zones: Vec::new(),
            boundary_polygon: Ring::must_new(vec![
                Pt2D::new(0.0, 0.0),
                Pt2D::new(1.0, 0.0),
                Pt2D::new(1.0, 1.0),
                Pt2D::new(0.0, 0.0),
            ])
            .into_polygon(),
            stop_signs: BTreeMap::new(),
            traffic_signals: BTreeMap::new(),
            bus_routes_on_roads: MultiMap::new(),
            gps_bounds: GPSBounds::new(),
            bounds: Bounds::new(),
            config: MapConfig::default(),
            pathfinder: Pathfinder::empty(),
            pathfinder_dirty: false,
            routing_params: RoutingParams::default(),
            name: MapName::blank(),
            edits: MapEdits::new(),
            edits_generation: 0,
            road_to_buildings: MultiMap::new(),
        }
    }

    /// A dummy map that won't crash UIs, but has almost nothing in it.
    pub fn almost_blank() -> Self {
        // Programatically creating a Map is very verbose. RawMap less so, but .osm could be even
        // better... but then we'd pull in dependencies for XML parsing everywhere.
        let mut raw = RawMap::blank(MapName::blank());

        raw.streets.boundary_polygon = Polygon::rectangle(100.0, 100.0);
        raw.streets
            .gps_bounds
            .update(LonLat::new(-122.453224, 47.723277));
        raw.streets
            .gps_bounds
            .update(LonLat::new(-122.240505, 47.495342));

        let i1 = raw.streets.insert_intersection(
            Vec::new(),
            Pt2D::new(30.0, 30.0),
            IntersectionKind::MapEdge,
            IntersectionControl::Uncontrolled,
        );
        let i2 = raw.streets.insert_intersection(
            Vec::new(),
            Pt2D::new(70.0, 70.0),
            IntersectionKind::MapEdge,
            IntersectionControl::Uncontrolled,
        );
        raw.elevation_per_intersection.insert(i1, Distance::ZERO);
        raw.elevation_per_intersection.insert(i2, Distance::ZERO);

        let mut tags = Tags::empty();
        tags.insert("highway", "residential");
        tags.insert("lanes", "2");
        let road_id = raw.streets.next_road_id();
        raw.streets.insert_road(osm2streets::Road::new(
            road_id,
            Vec::new(),
            i1,
            i2,
            PolyLine::must_new(vec![Pt2D::new(30.0, 30.0), Pt2D::new(70.0, 70.0)]),
            tags,
            &raw.streets.config,
        ));
        raw.extra_road_data
            .insert(road_id, raw_map::ExtraRoadData::default());

        raw.buildings.insert(
            osm::OsmID::Way(osm::WayID(3)),
            RawBuilding {
                polygon: Polygon::rectangle_centered(
                    Pt2D::new(50.0, 20.0),
                    Distance::meters(30.0),
                    Distance::meters(10.0),
                ),
                osm_tags: Tags::empty(),
                public_garage_name: None,
                num_parking_spots: 0,
                amenities: Vec::new(),
            },
        );

        Self::create_from_raw(
            raw,
            crate::RawToMapOptions::default(),
            &mut Timer::throwaway(),
        )
    }

    pub fn all_roads(&self) -> &Vec<Road> {
        &self.roads
    }

    pub fn all_lanes(&self) -> impl Iterator<Item = &Lane> {
        self.roads.iter().flat_map(|r| r.lanes.iter())
    }

    pub fn all_intersections(&self) -> &Vec<Intersection> {
        &self.intersections
    }

    pub fn all_turns(&self) -> impl Iterator<Item = &Turn> {
        self.intersections.iter().flat_map(|i| i.turns.iter())
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
        self.maybe_get_r(id.road)?.lanes.get(id.offset)
    }

    pub fn maybe_get_i(&self, id: IntersectionID) -> Option<&Intersection> {
        self.intersections.get(id.0)
    }

    pub fn maybe_get_t(&self, id: TurnID) -> Option<&Turn> {
        // Looking up the intersection is fast. Linearly scanning through all of the turns to find
        // this one actually turns out to be fast too; thanks cache locality.
        for turn in &self.intersections[id.parent.0].turns {
            if turn.id == id {
                return Some(turn);
            }
        }
        None
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

    pub fn maybe_get_ts(&self, id: TransitStopID) -> Option<&TransitStop> {
        self.transit_stops.get(&id)
    }

    pub fn maybe_get_stop_sign(&self, id: IntersectionID) -> Option<&ControlStopSign> {
        self.stop_signs.get(&id)
    }

    pub fn maybe_get_traffic_signal(&self, id: IntersectionID) -> Option<&ControlTrafficSignal> {
        self.traffic_signals.get(&id)
    }

    pub fn maybe_get_tr(&self, route: TransitRouteID) -> Option<&TransitRoute> {
        self.transit_routes.get(route.0)
    }

    pub fn get_r(&self, id: RoadID) -> &Road {
        &self.roads[id.0]
    }

    pub fn get_l(&self, id: LaneID) -> &Lane {
        &self.roads[id.road.0].lanes[id.offset]
    }

    pub(crate) fn mut_lane(&mut self, id: LaneID) -> &mut Lane {
        &mut self.roads[id.road.0].lanes[id.offset]
    }
    /// Public for importer. Do not abuse!
    pub fn mut_road(&mut self, id: RoadID) -> &mut Road {
        &mut self.roads[id.0]
    }
    pub(crate) fn mut_turn(&mut self, id: TurnID) -> &mut Turn {
        for turn in &mut self.intersections[id.parent.0].turns {
            if turn.id == id {
                return turn;
            }
        }
        panic!("Couldn't find {id}");
    }

    pub fn get_i(&self, id: IntersectionID) -> &Intersection {
        &self.intersections[id.0]
    }

    pub fn get_t(&self, id: TurnID) -> &Turn {
        // When pathfinding breaks, seeing this TurnID is useful.
        if let Some(turn) = self.maybe_get_t(id) {
            turn
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

    /// This will return None for SharedSidewalkCorners
    pub fn get_movement(&self, id: MovementID) -> Option<&Movement> {
        self.get_i(id.parent).movements.get(&id)
    }

    // All these helpers should take IDs and return objects.

    /// The turns may belong to two different intersections!
    pub fn get_turns_from_lane(&self, l: LaneID) -> Vec<&Turn> {
        let lane = self.get_l(l);
        self.get_i(lane.dst_i)
            .turns
            .iter()
            // Sidewalks are bidirectional, so include turns from the source intersection
            .chain(
                self.get_i(lane.src_i)
                    .turns
                    .iter()
                    // And then don't yield them if this isn't a sidewalk
                    .take_while(|_| lane.is_walkable()),
            )
            .filter(|t| t.id.src == l || (lane.is_walkable() && t.id.dst == l))
            .collect()
    }

    pub fn get_turns_to_lane(&self, l: LaneID) -> Vec<&Turn> {
        let lane = self.get_l(l);

        // Sidewalks/shoulders are bidirectional
        if lane.is_walkable() {
            return self.get_turns_from_lane(l);
        }

        let turns: Vec<&Turn> = self
            .get_i(lane.src_i)
            .turns
            .iter()
            .filter(|t| t.id.dst == l)
            .collect();
        turns
    }

    pub fn get_turn_between(
        &self,
        from: LaneID,
        to: LaneID,
        parent: IntersectionID,
    ) -> Option<&Turn> {
        self.get_i(parent)
            .turns
            .iter()
            .find(|t| t.id.src == from && t.id.dst == to)
    }

    pub fn get_next_turns_and_lanes(&self, from: LaneID) -> Vec<(&Turn, &Lane)> {
        self.get_turns_from_lane(from)
            .into_iter()
            .map(|t| {
                (
                    t,
                    self.get_l(if t.id.src == from { t.id.dst } else { t.id.src }),
                )
            })
            .collect()
    }

    pub fn get_next_turns_and_lanes_for(
        &self,
        from: LaneID,
        constraints: PathConstraints,
    ) -> Vec<(&Turn, &Lane)> {
        self.get_next_turns_and_lanes(from)
            .into_iter()
            .filter(|(_, l)| constraints.can_use(l, self))
            .collect()
    }

    pub fn get_turns_for(&self, from: LaneID, constraints: PathConstraints) -> Vec<&Turn> {
        self.get_next_turns_and_lanes_for(from, constraints)
            .into_iter()
            .map(|(t, _)| t)
            .collect()
    }

    /// Find all movements from one road to another that're usable by someone.
    pub fn get_movements_for(
        &self,
        from: DirectedRoadID,
        constraints: PathConstraints,
    ) -> Vec<MovementID> {
        let mut result = BTreeSet::new();
        for t in &self.get_i(from.dst_i(self)).turns {
            let src = self.get_l(t.id.src);
            if src.get_directed_parent() == from
                && constraints.can_use(src, self)
                && constraints.can_use(self.get_l(t.id.dst), self)
            {
                result.insert(t.id.to_movement(self));
            }
        }
        // TODO Sidewalks are bidirectional
        assert!(constraints != PathConstraints::Pedestrian);
        result.into_iter().collect()
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
        self.get_r(id.road)
    }

    pub fn get_gps_bounds(&self) -> &GPSBounds {
        &self.gps_bounds
    }

    pub fn get_bounds(&self) -> &Bounds {
        &self.bounds
    }

    pub fn get_city_name(&self) -> &CityName {
        &self.name.city
    }

    pub fn get_name(&self) -> &MapName {
        &self.name
    }

    pub fn all_transit_stops(&self) -> &BTreeMap<TransitStopID, TransitStop> {
        &self.transit_stops
    }

    pub fn get_ts(&self, stop: TransitStopID) -> &TransitStop {
        &self.transit_stops[&stop]
    }

    pub fn get_tr(&self, route: TransitRouteID) -> &TransitRoute {
        &self.transit_routes[route.0]
    }

    pub fn all_transit_routes(&self) -> &Vec<TransitRoute> {
        &self.transit_routes
    }

    pub fn get_transit_route(&self, name: &str) -> Option<&TransitRoute> {
        self.transit_routes.iter().find(|r| r.long_name == name)
    }

    pub fn get_routes_serving_stop(&self, stop: TransitStopID) -> Vec<&TransitRoute> {
        let mut routes = Vec::new();
        for r in &self.transit_routes {
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
        if let Some(l) = self
            .get_parent(sidewalk)
            .find_closest_lane(sidewalk, |l| PathConstraints::Car.can_use(l, self))
        {
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
                if lt == LaneType::Driving && !self.get_l(l).driving_blackhole {
                    return l;
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

    pub fn get_pathfinder(&self) -> &Pathfinder {
        &self.pathfinder
    }

    pub fn pathfind(&self, req: PathRequest) -> Result<Path> {
        self.pathfind_v2(req)?.into_v1(self)
    }
    pub fn pathfind_with_params(
        &self,
        req: PathRequest,
        params: &RoutingParams,
        cache_custom: PathfinderCaching,
    ) -> Result<Path> {
        self.pathfind_v2_with_params(req, params, cache_custom)?
            .into_v1(self)
    }
    pub fn pathfind_v2(&self, req: PathRequest) -> Result<PathV2> {
        assert!(!self.pathfinder_dirty);
        self.pathfinder
            .pathfind(req.clone(), self)
            .ok_or_else(|| anyhow!("can't fulfill {}", req))
    }
    pub fn pathfind_v2_with_params(
        &self,
        req: PathRequest,
        params: &RoutingParams,
        cache_custom: PathfinderCaching,
    ) -> Result<PathV2> {
        assert!(!self.pathfinder_dirty);
        self.pathfinder
            .pathfind_with_params(req.clone(), params, cache_custom, self)
            .ok_or_else(|| anyhow!("can't fulfill {}", req))
    }
    pub fn should_use_transit(
        &self,
        start: Position,
        end: Position,
    ) -> Option<(TransitStopID, Option<TransitStopID>, TransitRouteID)> {
        assert!(!self.pathfinder_dirty);
        self.pathfinder.should_use_transit(self, start, end)
    }

    /// Return the cost of a single path, and also a mapping from every directed road to the cost
    /// of getting there from the same start. This can be used to understand why an alternative
    /// route wasn't chosen.
    pub fn all_costs_from(
        &self,
        req: PathRequest,
    ) -> Option<(Duration, HashMap<DirectedRoadID, Duration>)> {
        assert!(!self.pathfinder_dirty);
        self.pathfinder.all_costs_from(req, self)
    }

    /// None for SharedSidewalkCorners and turns not belonging to traffic signals
    pub fn get_movement_for_traffic_signal(
        &self,
        t: TurnID,
    ) -> Option<(MovementID, CompressedMovementID)> {
        let i = self.get_i(t.parent);
        if !i.is_traffic_signal() || self.get_t(t).turn_type == TurnType::SharedSidewalkCorner {
            return None;
        }
        Some(i.turn_to_movement(t))
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

    pub fn find_tr_by_gtfs(&self, gtfs_id: &str) -> Option<TransitRouteID> {
        for tr in self.all_transit_routes() {
            if tr.gtfs_id == gtfs_id {
                return Some(tr.id);
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

    pub fn hack_override_orig_spawn_times(&mut self, br: TransitRouteID, times: Vec<Time>) {
        self.transit_routes[br.0].orig_spawn_times = times.clone();
        self.transit_routes[br.0].spawn_times = times;
    }

    pub fn hack_add_area(&mut self, area_type: AreaType, polygon: Polygon, osm_tags: Tags) {
        self.areas.push(Area {
            id: AreaID(self.areas.len()),
            area_type,
            polygon,
            osm_tags,
            osm_id: None,
        });
    }

    /// Normally after applying edits, you must call `recalculate_pathfinding_after_edits`.
    /// Alternatively, you can keep the old pathfinder exactly as it is. Use with caution -- the
    /// pathfinder and the map may be out-of-sync in arbitrary ways.
    pub fn keep_pathfinder_despite_edits(&mut self) {
        self.pathfinder_dirty = false;
    }

    pub fn get_languages(&self) -> BTreeSet<String> {
        let mut languages = BTreeSet::new();
        for r in self.all_roads() {
            for key in r.osm_tags.inner().keys() {
                if let Some(x) = key.strip_prefix("name:") {
                    languages.insert(x.to_string());
                }
            }
        }
        for b in self.all_buildings() {
            for a in &b.amenities {
                for lang in a.names.languages() {
                    languages.insert(lang.to_string());
                }
            }
        }
        languages
    }

    pub fn get_config(&self) -> &MapConfig {
        &self.config
    }

    /// Simple search along undirected roads. Expresses the result as a sequence of roads and a
    /// sequence of intersections.
    pub fn simple_path_btwn(
        &self,
        i1: IntersectionID,
        i2: IntersectionID,
    ) -> Option<(Vec<RoadID>, Vec<IntersectionID>)> {
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
            |(_, _, r)| self.get_r(*r).length(),
            |_| Distance::ZERO,
        )?;
        let roads: Vec<RoadID> = path
            .windows(2)
            .map(|pair| *graph.edge_weight(pair[0], pair[1]).unwrap())
            .collect();
        Some((roads, path))
    }

    /// Simple search along directed roads, weighted by distance. Expresses the result as a
    /// sequence of roads and a sequence of intersections.
    ///
    /// Unlike the main pathfinding methods, this starts and ends at intersections. The first and
    /// last step can be on any road connected to the intersection.
    // TODO Remove simple_path_btwn in favor of this one?
    pub fn simple_path_btwn_v2(
        &self,
        i1: IntersectionID,
        i2: IntersectionID,
        constraints: PathConstraints,
    ) -> Option<(Vec<RoadID>, Vec<IntersectionID>)> {
        let mut graph: DiGraphMap<IntersectionID, RoadID> = DiGraphMap::new();
        for r in self.all_roads() {
            let mut fwd = false;
            let mut back = false;
            for lane in &r.lanes {
                if constraints.can_use(lane, self) {
                    if lane.dir == Direction::Fwd {
                        fwd = true;
                    } else {
                        back = true;
                    }
                }
            }
            if fwd {
                graph.add_edge(r.src_i, r.dst_i, r.id);
            }
            if back {
                graph.add_edge(r.dst_i, r.src_i, r.id);
            }
        }
        let (_, path) = petgraph::algo::astar(
            &graph,
            i1,
            |i| i == i2,
            |(_, _, r)| self.get_r(*r).length(),
            |_| Distance::ZERO,
        )?;
        let roads: Vec<RoadID> = path
            .windows(2)
            .map(|pair| *graph.edge_weight(pair[0], pair[1]).unwrap())
            .collect();
        Some((roads, path))
    }

    /// Returns the routing params baked into the map.
    // Depending how this works out, we might require everybody to explicitly plumb routing params,
    // in which case it should be easy to look for all places calling this.
    pub fn routing_params(&self) -> &RoutingParams {
        &self.routing_params
    }

    pub fn road_to_buildings(&self, r: RoadID) -> &BTreeSet<BuildingID> {
        self.road_to_buildings.get(r)
    }

    pub(crate) fn recalculate_road_to_buildings(&mut self) {
        let mut mapping = MultiMap::new();
        for b in self.all_buildings() {
            mapping.insert(b.sidewalk_pos.lane().road, b.id);
        }
        self.road_to_buildings = mapping;
    }

    pub(crate) fn recalculate_all_movements(&mut self, timer: &mut Timer) {
        let movements = timer.parallelize(
            "generate movements",
            self.intersections.iter().map(|i| i.id).collect(),
            |i| Movement::for_i(i, self),
        );
        for (i, movements) in self.intersections.iter_mut().zip(movements.into_iter()) {
            i.movements = movements;
        }
    }

    /// Finds the road directly connecting two intersections.
    pub fn find_road_between(&self, i1: IntersectionID, i2: IntersectionID) -> Option<RoadID> {
        for r in &self.get_i(i1).roads {
            let road = self.get_r(*r);
            if road.src_i == i2 || road.dst_i == i2 {
                return Some(road.id);
            }
        }
        None
    }

    /// Returns the highest elevation in the map
    pub fn max_elevation(&self) -> Distance {
        // TODO Cache?
        self.all_intersections()
            .iter()
            .max_by_key(|i| i.elevation)
            .unwrap()
            .elevation
    }

    /// Does a turn at a stop sign go from a smaller to a larger road?
    /// (Note this doesn't look at unprotected movements in traffic signals, since we don't yet
    /// have good heuristics for when those exist)
    pub fn is_unprotected_turn(&self, from: RoadID, to: RoadID, turn_type: TurnType) -> bool {
        let unprotected_turn_type = if self.get_config().driving_side == DrivingSide::Right {
            TurnType::Left
        } else {
            TurnType::Right
        };
        let from = self.get_r(from);
        let to = self.get_r(to);
        turn_type == unprotected_turn_type
            && from.get_detailed_rank() < to.get_detailed_rank()
            && match from.common_endpoint(to) {
                CommonEndpoint::One(i) => self.get_i(i).is_stop_sign(),
                _ => false,
            }
    }

    /// Modifies the map in-place, removing parts not essential for the bike network tool.
    pub fn minify(&mut self, timer: &mut Timer) {
        // We only need the CHs for driving and biking, to support mode shift.
        self.pathfinder = Pathfinder::new_limited(
            self,
            self.routing_params().clone(),
            crate::pathfind::CreateEngine::CH,
            vec![PathConstraints::Car, PathConstraints::Bike],
            timer,
        );

        // Remove all routes, since we remove that pathfinder
        self.transit_stops.clear();
        self.transit_routes.clear();
        for r in &mut self.roads {
            r.transit_stops.clear();
        }
    }

    /// Modifies the map in-place, removing buildings.
    pub fn minify_buildings(&mut self, timer: &mut Timer) {
        self.buildings.clear();

        // We only need the CHs for driving.
        self.pathfinder = Pathfinder::new_limited(
            self,
            self.routing_params().clone(),
            crate::pathfind::CreateEngine::CH,
            vec![PathConstraints::Car],
            timer,
        );

        // Remove all routes, since we remove that pathfinder
        self.transit_stops.clear();
        self.transit_routes.clear();
        for r in &mut self.roads {
            r.transit_stops.clear();
        }
    }

    /// Export all road and intersection geometry to GeoJSON, transforming to WGS84
    pub fn export_geometry(&self) -> geojson::GeoJson {
        let mut pairs = Vec::new();
        let gps_bounds = Some(self.get_gps_bounds());

        for i in self.all_intersections() {
            let mut props = serde_json::Map::new();
            props.insert("type".to_string(), "intersection".into());
            props.insert("id".to_string(), i.orig_id.to_string().into());
            pairs.push((i.polygon.get_outer_ring().to_geojson(gps_bounds), props));
        }
        for r in self.all_roads() {
            let mut props = serde_json::Map::new();
            props.insert("type".to_string(), "road".into());
            props.insert("id".to_string(), r.orig_id.osm_way_id.to_string().into());
            pairs.push((
                r.center_pts
                    .to_thick_ring(r.get_width())
                    .to_geojson(gps_bounds),
                props,
            ));
        }

        geom::geometries_with_properties_to_geojson(pairs)
    }

    /// What're the names of bus routes along a road? Note this is best effort, not robust to edits
    /// or transformations.
    pub fn get_bus_routes_on_road(&self, r: RoadID) -> &BTreeSet<String> {
        let way = self.get_r(r).orig_id.osm_way_id;
        self.bus_routes_on_roads.get(way)
    }

    /// Find all amenity types that at least 1 building contains
    pub fn get_available_amenity_types(&self) -> BTreeSet<AmenityType> {
        let mut result = BTreeSet::new();
        for b in self.all_buildings() {
            for amenity in &b.amenities {
                if let Some(t) = AmenityType::categorize(&amenity.amenity_type) {
                    result.insert(t);
                }
            }
        }
        result
    }
}
