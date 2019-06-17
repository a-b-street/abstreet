use crate::make::get_lane_types;
use crate::pathfind::Pathfinder;
use crate::{
    make, raw_data, Area, AreaID, Building, BuildingID, BusRoute, BusRouteID, BusStop, BusStopID,
    ControlStopSign, ControlTrafficSignal, Intersection, IntersectionID, IntersectionType, Lane,
    LaneID, LaneType, MapEdits, Path, PathRequest, Position, Road, RoadID, Turn, TurnID,
    TurnPriority,
};
use abstutil;
use abstutil::{deserialize_btreemap, serialize_btreemap, Error, Timer};
use geom::{Bounds, GPSBounds, Polygon};
use serde_derive::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashSet, VecDeque};
use std::io;

#[derive(Serialize, Deserialize)]
pub struct Map {
    roads: Vec<Road>,
    lanes: Vec<Lane>,
    intersections: Vec<Intersection>,
    #[serde(
        serialize_with = "serialize_btreemap",
        deserialize_with = "deserialize_btreemap"
    )]
    turns: BTreeMap<TurnID, Turn>,
    buildings: Vec<Building>,
    #[serde(
        serialize_with = "serialize_btreemap",
        deserialize_with = "deserialize_btreemap"
    )]
    bus_stops: BTreeMap<BusStopID, BusStop>,
    bus_routes: Vec<BusRoute>,
    areas: Vec<Area>,
    boundary_polygon: Polygon,

    // Note that border nodes belong in neither!
    stop_signs: BTreeMap<IntersectionID, ControlStopSign>,
    traffic_signals: BTreeMap<IntersectionID, ControlTrafficSignal>,

    gps_bounds: GPSBounds,
    bounds: Bounds,

    turn_lookup: Vec<TurnID>,
    // TODO Argh, hack, initialization order is hard!
    pathfinder: Option<Pathfinder>,

    name: String,
    edits: MapEdits,
}

impl Map {
    pub fn new(path: &str, timer: &mut Timer) -> Result<Map, io::Error> {
        let data: raw_data::Map = abstutil::read_binary(path, timer)?;
        Ok(Map::create_from_raw(abstutil::basename(path), data, timer))
    }

    pub fn create_from_raw(name: String, data: raw_data::Map, timer: &mut Timer) -> Map {
        timer.start("raw_map to InitialMap");
        let gps_bounds = data.gps_bounds.clone();
        let bounds = gps_bounds.to_bounds();
        let mut initial_map =
            make::InitialMap::new(name.clone(), &data, &gps_bounds, &bounds, timer);
        let hints = raw_data::Hints::load();
        initial_map.apply_hints(&hints, &data, timer);
        timer.stop("raw_map to InitialMap");

        timer.start("InitialMap to HalfMap");
        let half_map = make::make_half_map(&data, initial_map, &gps_bounds, &bounds, timer);
        timer.stop("InitialMap to HalfMap");

        timer.start("finalize Map");
        let mut m = Map {
            roads: half_map.roads,
            lanes: half_map.lanes,
            intersections: half_map.intersections,
            turns: half_map.turns,
            buildings: half_map.buildings,
            bus_stops: BTreeMap::new(),
            bus_routes: Vec::new(),
            areas: half_map.areas,
            boundary_polygon: Polygon::new(&gps_bounds.must_convert(&data.boundary_polygon)),
            stop_signs: BTreeMap::new(),
            traffic_signals: BTreeMap::new(),
            gps_bounds,
            bounds,
            turn_lookup: half_map.turn_lookup,
            pathfinder: None,
            name: name.clone(),
            edits: MapEdits::new(name),
        };

        // Extra setup that's annoying to do as HalfMap, since we want to pass around a Map.
        {
            let mut stop_signs: BTreeMap<IntersectionID, ControlStopSign> = BTreeMap::new();
            let mut traffic_signals: BTreeMap<IntersectionID, ControlTrafficSignal> =
                BTreeMap::new();
            for i in &m.intersections {
                match i.intersection_type {
                    IntersectionType::StopSign => {
                        stop_signs.insert(i.id, ControlStopSign::new(&m, i.id, timer));
                    }
                    IntersectionType::TrafficSignal => {
                        traffic_signals.insert(i.id, ControlTrafficSignal::new(&m, i.id, timer));
                    }
                    IntersectionType::Border => {}
                };
            }
            m.stop_signs = stop_signs;
            m.traffic_signals = traffic_signals;
        }

        // Here's a fun one: we can't set up walking_using_transit yet, because we haven't
        // finalized bus stops and routes. We need the bus graph in place for that. So setup
        // pathfinding in two stages.
        timer.start("setup (most of) Pathfinder");
        m.pathfinder = Some(Pathfinder::new_without_transit(&m, timer));
        timer.stop("setup (most of) Pathfinder");

        {
            let (stops, routes) =
                make::make_bus_stops(&m, &data.bus_routes, &m.gps_bounds, &m.bounds, timer);
            m.bus_stops = stops;
            // The IDs are sorted in the BTreeMap, so this order winds up correct.
            for id in m.bus_stops.keys() {
                m.lanes[id.sidewalk.0].bus_stops.push(*id);
            }

            m.bus_routes = make::verify_bus_routes(&m, routes, timer);
        }

        timer.start("setup rest of Pathfinder");
        let mut pathfinder = m.pathfinder.take().unwrap();
        pathfinder.setup_walking_with_transit(&m);
        m.pathfinder = Some(pathfinder);
        timer.stop("setup rest of Pathfinder");

        timer.stop("finalize Map");
        m
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
        &self.turns[&id]
    }

    pub fn get_b(&self, id: BuildingID) -> &Building {
        &self.buildings[id.0]
    }

    pub fn get_a(&self, id: AreaID) -> &Area {
        &self.areas[id.0]
    }

    pub fn get_stop_sign(&self, id: IntersectionID) -> &ControlStopSign {
        &self.stop_signs[&id]
    }

    pub fn get_traffic_signal(&self, id: IntersectionID) -> &ControlTrafficSignal {
        &self.traffic_signals[&id]
    }

    pub fn lookup_turn_by_idx(&self, idx: usize) -> Option<TurnID> {
        self.turn_lookup.get(idx).cloned()
    }

    // All these helpers should take IDs and return objects.

    pub fn get_source_intersection(&self, l: LaneID) -> &Intersection {
        self.get_i(self.get_l(l).src_i)
    }

    pub fn get_destination_intersection(&self, l: LaneID) -> &Intersection {
        self.get_i(self.get_l(l).dst_i)
    }

    pub fn get_turns_in_intersection(&self, id: IntersectionID) -> Vec<&Turn> {
        self.get_i(id)
            .turns
            .iter()
            .map(|t| self.get_t(*t))
            .collect()
    }

    // TODO Get rid of this, or rewrite it in in terms of get_next_turns_and_lanes
    // The turns may belong to two different intersections!
    pub fn get_turns_from_lane(&self, l: LaneID) -> Vec<&Turn> {
        let lane = self.get_l(l);
        let mut turns: Vec<&Turn> = self
            .get_i(lane.dst_i)
            .turns
            .iter()
            .map(|t| self.get_t(*t))
            .filter(|t| t.id.src == l)
            .collect();
        // Sidewalks are bidirectional
        if lane.is_sidewalk() {
            for t in &self.get_i(lane.src_i).turns {
                if t.src == l {
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

    // These come back sorted
    pub fn get_next_roads(&self, from: RoadID) -> Vec<RoadID> {
        let mut roads: BTreeSet<RoadID> = BTreeSet::new();

        let r = self.get_r(from);
        for id in vec![r.src_i, r.dst_i].into_iter() {
            roads.extend(self.get_i(id).roads.clone());
        }

        roads.into_iter().collect()
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

    pub fn get_name(&self) -> &String {
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

    pub fn get_all_bus_routes(&self) -> &Vec<BusRoute> {
        &self.bus_routes
    }

    pub fn get_bus_route(&self, name: &str) -> Option<&BusRoute> {
        self.bus_routes.iter().find(|r| r.name == name)
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

    // This and all_outgoing_borders are expensive to constantly repeat
    pub fn all_incoming_borders(&self) -> Vec<&Intersection> {
        let mut result: Vec<&Intersection> = Vec::new();
        for i in &self.intersections {
            if i.intersection_type == IntersectionType::Border && !i.outgoing_lanes.is_empty() {
                result.push(i);
            }
        }
        result
    }

    pub fn all_outgoing_borders(&self) -> Vec<&Intersection> {
        let mut result: Vec<&Intersection> = Vec::new();
        for i in &self.intersections {
            if i.intersection_type == IntersectionType::Border && !i.incoming_lanes.is_empty() {
                result.push(i);
            }
        }
        result
    }

    pub fn save(&self) {
        assert_eq!(self.edits.edits_name, "no_edits");
        let path = format!("../data/maps/{}.bin", self.name);
        println!("Saving {}...", path);
        abstutil::write_binary(&path, self).expect(&format!("Saving {} failed", path));
        println!("Saved {}", path);
    }

    pub fn find_closest_lane(&self, from: LaneID, types: Vec<LaneType>) -> Result<LaneID, Error> {
        self.get_parent(from).find_closest_lane(from, types)
    }

    pub fn find_closest_lane_to_bldg(
        &self,
        bldg: BuildingID,
        types: Vec<LaneType>,
    ) -> Result<LaneID, Error> {
        let from = self.get_b(bldg).sidewalk();
        self.find_closest_lane(from, types)
    }

    // TODO reconsider names, or put somewhere else?
    pub fn intersection(&self, label: &str) -> &Intersection {
        for i in &self.intersections {
            if i.label == Some(label.to_string()) {
                return i;
            }
        }
        panic!("No intersection has label {}", label);
    }

    pub fn bldg(&self, label: &str) -> &Building {
        for b in &self.buildings {
            if b.osm_tags.get("label") == Some(&label.to_string()) {
                return b;
            }
        }
        panic!("No building has label {}", label);
    }

    pub fn driving_lane(&self, label: &str) -> &Lane {
        for l in &self.lanes {
            if !l.is_driving() {
                continue;
            }
            let r = self.get_parent(l.id);
            if (r.is_forwards(l.id) && r.osm_tags.get("fwd_label") == Some(&label.to_string()))
                || (r.is_backwards(l.id)
                    && r.osm_tags.get("back_label") == Some(&label.to_string()))
            {
                return l;
            }
        }
        panic!("No driving lane has label {}", label);
    }

    pub fn parking_lane(&self, label: &str, expected_spots: usize) -> &Lane {
        for l in &self.lanes {
            if !l.is_parking() {
                continue;
            }
            let r = self.get_parent(l.id);
            if (r.is_forwards(l.id) && r.osm_tags.get("fwd_label") == Some(&label.to_string()))
                || (r.is_backwards(l.id)
                    && r.osm_tags.get("back_label") == Some(&label.to_string()))
            {
                let actual_spots = l.number_parking_spots();
                if expected_spots != actual_spots {
                    panic!(
                        "Parking lane {} (labeled {}) has {} spots, not {}",
                        l.id, label, actual_spots, expected_spots
                    );
                }
                return l;
            }
        }
        panic!("No parking lane has label {}", label);
    }

    pub fn is_turn_allowed(&self, t: TurnID) -> bool {
        if let Some(ss) = self.stop_signs.get(&t.parent) {
            ss.get_priority(t) != TurnPriority::Banned
        } else if let Some(ts) = self.traffic_signals.get(&t.parent) {
            ts.cycles
                .iter()
                .any(|c| c.get_priority(t) != TurnPriority::Banned)
        } else {
            // Border nodes have no turns...
            panic!("{}'s intersection isn't a stop sign or traffic signal", t);
        }
    }

    // When driving towards some goal building, there may not be a driving lane directly outside the
    // building. So BFS out in a deterministic way and find one.
    pub fn find_driving_lane_near_building(&self, b: BuildingID) -> LaneID {
        if let Ok(l) = self.find_closest_lane_to_bldg(b, vec![LaneType::Driving]) {
            return l;
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

            for (lane, lane_type) in r
                .children_forwards
                .iter()
                .chain(r.children_backwards.iter())
            {
                if *lane_type == LaneType::Driving {
                    return *lane;
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

    pub fn pathfind(&self, req: PathRequest) -> Option<Path> {
        self.pathfinder.as_ref().unwrap().pathfind(req, self)
    }

    pub fn should_use_transit(
        &self,
        start: Position,
        end: Position,
    ) -> Option<(BusStopID, BusStopID, BusRouteID)> {
        self.pathfinder
            .as_ref()
            .unwrap()
            .should_use_transit(self, start, end)
    }
}

impl Map {
    pub fn get_edits(&self) -> &MapEdits {
        &self.edits
    }

    // new_edits assumed to be valid. Returns actual lanes that changed, turns deleted, turns added.
    pub fn apply_edits(
        &mut self,
        new_edits: MapEdits,
        timer: &mut Timer,
    ) -> (BTreeSet<LaneID>, BTreeSet<TurnID>, BTreeSet<TurnID>) {
        // Ignore if there's no change from current
        let mut all_lane_edits: BTreeMap<LaneID, LaneType> = BTreeMap::new();
        let mut all_stop_sign_edits: BTreeMap<IntersectionID, ControlStopSign> = BTreeMap::new();
        let mut all_traffic_signals: BTreeMap<IntersectionID, ControlTrafficSignal> =
            BTreeMap::new();
        for (id, lt) in &new_edits.lane_overrides {
            if self.edits.lane_overrides.get(id) != Some(lt) {
                all_lane_edits.insert(*id, *lt);
            }
        }
        for (id, ss) in &new_edits.stop_sign_overrides {
            if self.edits.stop_sign_overrides.get(id) != Some(ss) {
                all_stop_sign_edits.insert(*id, ss.clone());
            }
        }
        for (id, ts) in &new_edits.traffic_signal_overrides {
            if self.edits.traffic_signal_overrides.get(id) != Some(ts) {
                all_traffic_signals.insert(*id, ts.clone());
            }
        }

        // May need to revert some previous changes
        for id in self.edits.lane_overrides.keys() {
            if !new_edits.lane_overrides.contains_key(id) {
                all_lane_edits.insert(*id, self.get_original_lt(*id));
            }
        }
        for id in self.edits.stop_sign_overrides.keys() {
            if !new_edits.stop_sign_overrides.contains_key(id) {
                all_stop_sign_edits.insert(*id, ControlStopSign::new(self, *id, timer));
            }
        }
        for id in self.edits.traffic_signal_overrides.keys() {
            if !new_edits.traffic_signal_overrides.contains_key(id) {
                all_traffic_signals.insert(*id, ControlTrafficSignal::new(self, *id, timer));
            }
        }

        timer.note(format!(
            "Total diff: {} lanes, {} stop signs, {} traffic signals",
            all_lane_edits.len(),
            all_stop_sign_edits.len(),
            all_traffic_signals.len()
        ));

        let mut changed_lanes = BTreeSet::new();
        let mut changed_intersections = BTreeSet::new();
        let mut changed_roads = BTreeSet::new();
        for (id, lt) in all_lane_edits {
            changed_lanes.insert(id);

            let l = &mut self.lanes[id.0];
            l.lane_type = lt;

            let r = &mut self.roads[l.parent.0];
            let (fwds, idx) = r.dir_and_offset(l.id);
            if fwds {
                r.children_forwards[idx] = (l.id, l.lane_type);
            } else {
                r.children_backwards[idx] = (l.id, l.lane_type);
            }

            changed_intersections.insert(l.src_i);
            changed_intersections.insert(l.dst_i);
            changed_roads.insert(l.parent);
        }

        for id in changed_roads {
            let stops = self.get_r(id).all_bus_stops(self);
            for s in stops {
                let sidewalk_pos = self.get_bs(s).sidewalk_pos;
                // Must exist, because we aren't allowed to orphan a bus stop.
                let driving_lane = self
                    .get_r(id)
                    .find_closest_lane(sidewalk_pos.lane(), vec![LaneType::Driving, LaneType::Bus])
                    .unwrap();
                let driving_pos = sidewalk_pos.equiv_pos(driving_lane, self);
                self.bus_stops.get_mut(&s).unwrap().driving_pos = driving_pos;
            }
        }

        // Recompute turns and intersection policy
        let mut delete_turns = BTreeSet::new();
        let mut add_turns = BTreeSet::new();
        for id in changed_intersections {
            let i = &mut self.intersections[id.0];

            if i.intersection_type == IntersectionType::Border {
                assert!(i.turns.is_empty());
                continue;
            }

            let mut old_turns = Vec::new();
            for id in i.turns.drain(..) {
                old_turns.push(self.turns.remove(&id).unwrap());
                delete_turns.insert(id);
            }

            for t in make::make_all_turns(i, &self.roads, &self.lanes, timer) {
                add_turns.insert(t.id);
                i.turns.push(t.id);
                if let Some(_existing_t) = old_turns.iter().find(|turn| turn.id == t.id) {
                    // TODO Except for lookup_idx
                    //assert_eq!(t, *existing_t);
                }
                self.turns.insert(t.id, t);
            }

            // TODO Deal with turn_lookup

            // Do this before applying intersection policy edits.
            match i.intersection_type {
                IntersectionType::StopSign => {
                    self.stop_signs
                        .insert(id, ControlStopSign::new(self, id, timer));
                }
                IntersectionType::TrafficSignal => {
                    self.traffic_signals
                        .insert(id, ControlTrafficSignal::new(self, id, timer));
                }
                IntersectionType::Border => unreachable!(),
            }
        }

        // Make sure all of the turns of modified intersections are re-added in the pathfinder;
        // they might've become banned. Lane markings may also change based on turn priorities.
        for (id, ss) in all_stop_sign_edits {
            self.stop_signs.insert(id, ss);
            for t in &self.get_i(id).turns {
                add_turns.insert(*t);
            }
            for l in &self.get_i(id).incoming_lanes {
                changed_lanes.insert(*l);
            }
        }
        for (id, ts) in all_traffic_signals {
            self.traffic_signals.insert(id, ts);
            for t in &self.get_i(id).turns {
                add_turns.insert(*t);
            }
            for l in &self.get_i(id).incoming_lanes {
                changed_lanes.insert(*l);
            }
        }

        let mut pathfinder = self.pathfinder.take().unwrap();
        pathfinder.apply_edits(&delete_turns, &add_turns, self, timer);
        self.pathfinder = Some(pathfinder);

        self.edits = new_edits;
        (changed_lanes, delete_turns, add_turns)
    }

    pub fn simplify_edits(&mut self, timer: &mut Timer) {
        let mut delete_lanes = Vec::new();
        for (id, lt) in &self.edits.lane_overrides {
            if *lt == self.get_original_lt(*id) {
                delete_lanes.push(*id);
            }
        }
        for id in delete_lanes {
            self.edits.lane_overrides.remove(&id);
        }

        let mut delete_stop_signs = Vec::new();
        for (id, ss) in &self.edits.stop_sign_overrides {
            if *ss == ControlStopSign::new(self, *id, timer) {
                delete_stop_signs.push(*id);
            }
        }
        for id in delete_stop_signs {
            self.edits.stop_sign_overrides.remove(&id);
        }

        let mut delete_signals = Vec::new();
        for (id, ts) in &self.edits.traffic_signal_overrides {
            if *ts == ControlTrafficSignal::new(self, *id, timer) {
                delete_signals.push(*id);
            }
        }
        for id in delete_signals {
            self.edits.traffic_signal_overrides.remove(&id);
        }
    }

    fn get_original_lt(&self, id: LaneID) -> LaneType {
        let parent = self.get_parent(id);
        let (side1, side2) = get_lane_types(
            &parent.osm_tags,
            parent.parking_lane_fwd,
            parent.parking_lane_back,
        );
        let (fwds, idx) = parent.dir_and_offset(id);
        if fwds {
            side1[idx]
        } else {
            side2[idx]
        }
    }
}
