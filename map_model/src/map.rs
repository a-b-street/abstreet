use crate::make::get_lane_types;
use crate::pathfind::Pathfinder;
use crate::{
    make, osm, raw_data, Area, AreaID, Building, BuildingID, BusRoute, BusRouteID, BusStop,
    BusStopID, ControlStopSign, ControlTrafficSignal, Intersection, IntersectionID,
    IntersectionType, Lane, LaneID, LaneType, MapEdits, Path, PathRequest, Position, Road, RoadID,
    Turn, TurnID, TurnPriority,
};
use abstutil;
use abstutil::{deserialize_btreemap, serialize_btreemap, Error, Timer};
use geom::{Bounds, GPSBounds, Polygon, Pt2D};
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
    pathfinder_dirty: bool,

    name: String,
    edits: MapEdits,
}

impl Map {
    pub fn new(path: &str, timer: &mut Timer) -> Result<Map, io::Error> {
        let mut data: raw_data::Map = abstutil::read_binary(path, timer)?;
        data.apply_fixes(&raw_data::MapFixes::load(), timer);
        // Do this after applying fixes, which might split off pieces of the map.
        make::remove_disconnected_roads(&mut data, timer);
        Ok(Map::create_from_raw(data, timer))
    }

    // Just for temporary std::mem::replace tricks.
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
            boundary_polygon: Polygon::new(&vec![
                Pt2D::new(0.0, 0.0),
                Pt2D::new(1.0, 0.0),
                Pt2D::new(1.0, 1.0),
            ]),
            stop_signs: BTreeMap::new(),
            traffic_signals: BTreeMap::new(),
            gps_bounds: GPSBounds::new(),
            bounds: Bounds::new(),
            turn_lookup: Vec::new(),
            pathfinder: None,
            pathfinder_dirty: false,
            name: "blank".to_string(),
            edits: MapEdits::new("blank".to_string()),
        }
    }

    fn create_from_raw(data: raw_data::Map, timer: &mut Timer) -> Map {
        timer.start("raw_map to InitialMap");
        let gps_bounds = data.gps_bounds.clone();
        let bounds = gps_bounds.to_bounds();
        let mut initial_map = make::InitialMap::new(data.name.clone(), &data, &bounds, timer);
        let hints = raw_data::Hints::load();
        initial_map.apply_hints(&hints, &data, timer);
        timer.stop("raw_map to InitialMap");

        timer.start("InitialMap to HalfMap");
        let half_map = make::make_half_map(&data, initial_map, &bounds, timer);
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
            boundary_polygon: data.boundary_polygon.clone(),
            stop_signs: BTreeMap::new(),
            traffic_signals: BTreeMap::new(),
            gps_bounds,
            bounds,
            turn_lookup: half_map.turn_lookup,
            pathfinder: None,
            pathfinder_dirty: false,
            name: data.name.clone(),
            edits: MapEdits::new(data.name),
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

            // Remove orphaned bus stops
            let mut remove_stops = HashSet::new();
            for id in m.bus_stops.keys() {
                if m.get_routes_serving_stop(*id).is_empty() {
                    remove_stops.insert(*id);
                }
            }
            for id in &remove_stops {
                m.bus_stops.remove(id);
                m.lanes[id.sidewalk.0]
                    .bus_stops
                    .retain(|stop| !remove_stops.contains(stop))
            }
        }

        timer.start("setup rest of Pathfinder (walking with transit)");
        let mut pathfinder = m.pathfinder.take().unwrap();
        pathfinder.setup_walking_with_transit(&m);
        m.pathfinder = Some(pathfinder);
        timer.stop("setup rest of Pathfinder (walking with transit)");

        timer.start("find parking blackholes");
        for (l, redirect) in make::redirect_parking_blackholes(&m, timer) {
            m.lanes[l.0].parking_blackhole = Some(redirect);
        }
        timer.stop("find parking blackholes");

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

    pub fn get_turns_to_lane(&self, l: LaneID) -> Vec<&Turn> {
        let lane = self.get_l(l);
        let mut turns: Vec<&Turn> = self
            .get_i(lane.src_i)
            .turns
            .iter()
            .map(|t| self.get_t(*t))
            .filter(|t| t.id.dst == l)
            .collect();
        // Sidewalks are bidirectional
        if lane.is_sidewalk() {
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

    pub fn get_legal_turns(&self, from: LaneID, lane_types: Vec<LaneType>) -> Vec<&Turn> {
        let valid_types: HashSet<LaneType> = lane_types.into_iter().collect();
        self.get_next_turns_and_lanes(from, self.get_l(from).dst_i)
            .into_iter()
            .filter(|(t, l)| self.is_turn_allowed(t.id) && valid_types.contains(&l.lane_type))
            .map(|(t, _)| t)
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
            if i.is_border() && !i.outgoing_lanes.is_empty() {
                result.push(i);
            }
        }
        result
    }

    pub fn all_outgoing_borders(&self) -> Vec<&Intersection> {
        let mut result: Vec<&Intersection> = Vec::new();
        for i in &self.intersections {
            if i.is_border() && !i.incoming_lanes.is_empty() {
                result.push(i);
            }
        }
        result
    }

    pub fn save(&self) {
        assert_eq!(self.edits.edits_name, "no_edits");
        assert!(!self.pathfinder_dirty);
        let path = abstutil::path_map(&self.name);
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
            if b.osm_tags.get(osm::LABEL) == Some(&label.to_string()) {
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
            if (r.is_forwards(l.id) && r.osm_tags.get(osm::FWD_LABEL) == Some(&label.to_string()))
                || (r.is_backwards(l.id)
                    && r.osm_tags.get(osm::BACK_LABEL) == Some(&label.to_string()))
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
            if (r.is_forwards(l.id) && r.osm_tags.get(osm::FWD_LABEL) == Some(&label.to_string()))
                || (r.is_backwards(l.id)
                    && r.osm_tags.get(osm::BACK_LABEL) == Some(&label.to_string()))
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

    // Cars trying to park near this building should head for the driving lane returned here, then
    // start their search. Some parking lanes are connected to driving lanes that're "parking
    // blackholes" -- if there are no free spots on that lane, then the roads force cars to a
    // border.
    pub fn find_driving_lane_near_building(&self, b: BuildingID) -> LaneID {
        if let Ok(l) = self.find_closest_lane_to_bldg(b, vec![LaneType::Driving]) {
            return self.get_l(l).parking_blackhole.unwrap_or(l);
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
                    return self.get_l(*lane).parking_blackhole.unwrap_or(*lane);
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
        assert!(!self.pathfinder_dirty);
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

    // new_edits assumed to be valid. Returns actual lanes that changed, turns deleted, turns added. Doesn't update pathfinding yet.
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

            if i.is_border() {
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

        self.edits = new_edits;
        self.pathfinder_dirty = true;
        (changed_lanes, delete_turns, add_turns)
    }

    pub fn recalculate_pathfinding_after_edits(&mut self, timer: &mut Timer) {
        if !self.pathfinder_dirty {
            return;
        }

        let mut pathfinder = self.pathfinder.take().unwrap();
        pathfinder.apply_edits(self, timer);
        self.pathfinder = Some(pathfinder);

        // Also recompute parking blackholes. This is cheap enough to do from scratch.
        timer.start("recompute parking blackholes");
        for l in self.lanes.iter_mut() {
            l.parking_blackhole = None;
        }
        for (l, redirect) in make::redirect_parking_blackholes(self, timer) {
            self.lanes[l.0].parking_blackhole = Some(redirect);
        }
        timer.stop("recompute parking blackholes");

        self.pathfinder_dirty = false;
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
