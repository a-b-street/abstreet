use crate::raw::{DrivingSide, RawMap};
use crate::{
    Area, AreaID, Building, BuildingID, BuildingType, BusRoute, BusRouteID, BusStop, BusStopID,
    ControlStopSign, ControlTrafficSignal, Intersection, IntersectionID, Lane, LaneID, LaneType,
    Map, MapEdits, OffstreetParking, ParkingLot, ParkingLotID, Path, PathConstraints, PathRequest,
    Position, Road, RoadID, Turn, TurnGroupID, TurnID, TurnType,
};
use abstutil::Timer;
use geom::{Angle, Bounds, Distance, GPSBounds, Line, PolyLine, Polygon, Pt2D, Ring};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashSet, VecDeque};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MapConfig {
    // If true, driving happens on the right side of the road (USA). If false, on the left
    // (Australia).
    pub driving_side: DrivingSide,
    pub bikes_can_use_bus_lanes: bool,
}

impl Map {
    pub fn new(path: String, timer: &mut Timer) -> Map {
        if path.starts_with(&abstutil::path_all_maps()) {
            match abstutil::maybe_read_binary(path.clone(), timer) {
                Ok(map) => {
                    let map: Map = map;

                    if false {
                        use abstutil::{prettyprint_usize, serialized_size_bytes};
                        println!(
                            "- {} roads: {} bytes",
                            prettyprint_usize(map.roads.len()),
                            prettyprint_usize(serialized_size_bytes(&map.roads))
                        );
                        println!(
                            "- {} lanes: {} bytes",
                            prettyprint_usize(map.lanes.len()),
                            prettyprint_usize(serialized_size_bytes(&map.lanes))
                        );
                        println!(
                            "- {} intersections: {} bytes",
                            prettyprint_usize(map.intersections.len()),
                            prettyprint_usize(serialized_size_bytes(&map.intersections))
                        );
                        println!(
                            "- {} turns: {} bytes",
                            prettyprint_usize(map.turns.len()),
                            prettyprint_usize(serialized_size_bytes(&map.turns))
                        );
                        println!(
                            "- {} buildings: {} bytes",
                            prettyprint_usize(map.buildings.len()),
                            prettyprint_usize(serialized_size_bytes(&map.buildings))
                        );
                        println!(
                            "- {} areas: {} bytes",
                            prettyprint_usize(map.areas.len()),
                            prettyprint_usize(serialized_size_bytes(&map.areas))
                        );
                        println!(
                            "- {} parking lots: {} bytes",
                            prettyprint_usize(map.parking_lots.len()),
                            prettyprint_usize(serialized_size_bytes(&map.parking_lots))
                        );
                        println!(
                            "- {} zones: {} bytes",
                            prettyprint_usize(map.zones.len()),
                            prettyprint_usize(serialized_size_bytes(&map.zones))
                        );
                        // This is the partridge in the pear tree, I suppose
                        println!(
                            "- pathfinder: {} bytes",
                            prettyprint_usize(serialized_size_bytes(&map.pathfinder))
                        );
                    }

                    return map;
                }
                Err(err) => {
                    Map::corrupt_err(path, err);
                    std::process::exit(1);
                }
            }
        }

        let raw: RawMap = if path.starts_with(&abstutil::path_all_raw_maps()) {
            abstutil::read_binary(path, timer)
        } else {
            // Synthetic
            abstutil::read_json(path, timer)
        };
        Map::create_from_raw(raw, true, timer)
    }

    pub fn corrupt_err(path: String, err: std::io::Error) {
        println!("\nError loading {}: {}\n", path, err);
        if err.to_string().contains("No such file") {
            println!(
                "{} is missing. You may need to do: cargo run --bin updater",
                path
            );
        } else {
            println!(
                "{} is out-of-date. You may need to update your build (git pull) or download new \
                 data (cargo run --bin updater). If this is a custom map, you need to import it \
                 again.",
                path
            );
        }
        println!(
            "Check https://github.com/dabreegster/abstreet/blob/master/docs/dev.md and file an \
             issue if you have trouble."
        );
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
            parking_lots: Vec::new(),
            zones: Vec::new(),
            boundary_polygon: Ring::must_new(vec![
                Pt2D::new(0.0, 0.0),
                Pt2D::new(1.0, 0.0),
                Pt2D::new(1.0, 1.0),
            ])
            .to_polygon(),
            stop_signs: BTreeMap::new(),
            traffic_signals: BTreeMap::new(),
            gps_bounds: GPSBounds::new(),
            bounds: Bounds::new(),
            config: MapConfig {
                driving_side: DrivingSide::Right,
                bikes_can_use_bus_lanes: true,
            },
            pathfinder: None,
            pathfinder_dirty: false,
            city_name: "blank city".to_string(),
            name: "blank".to_string(),
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

    pub fn get_turns_in_intersection<'a>(
        &'a self,
        id: IntersectionID,
    ) -> impl Iterator<Item = &'a Turn> + 'a {
        self.get_i(id).turns.iter().map(move |t| self.get_t(*t))
    }

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

    pub fn get_next_turns_and_lanes<'a>(
        &'a self,
        from: LaneID,
        parent: IntersectionID,
    ) -> impl Iterator<Item = (&'a Turn, &'a Lane)> + 'a {
        self.get_i(parent)
            .turns
            .iter()
            .filter(move |t| t.src == from)
            .map(move |t| (self.get_t(*t), self.get_l(t.dst)))
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

    // These come back sorted
    pub fn get_next_roads(&self, from: RoadID) -> impl Iterator<Item = RoadID> {
        let mut roads: BTreeSet<RoadID> = BTreeSet::new();

        let r = self.get_r(from);
        for id in vec![r.src_i, r.dst_i].into_iter() {
            roads.extend(self.get_i(id).roads.clone());
        }

        roads.into_iter()
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
        &self.city_name
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

    // This and all_outgoing_borders are expensive to constantly repeat
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

    pub fn unsaved_edits(&self) -> bool {
        self.edits.edits_name == "untitled edits" && !self.edits.commands.is_empty()
    }

    pub fn save(&self) {
        assert_eq!(self.edits.edits_name, "untitled edits");
        assert!(self.edits.commands.is_empty());
        assert!(!self.pathfinder_dirty);
        abstutil::write_binary(abstutil::path_map(&self.name), self);
    }

    pub fn find_closest_lane(
        &self,
        from: LaneID,
        types: Vec<LaneType>,
    ) -> Result<LaneID, Box<dyn std::error::Error>> {
        self.get_parent(from).find_closest_lane(from, types)
    }

    // Cars trying to park near this building should head for the driving lane returned here, then
    // start their search. Some parking lanes are connected to driving lanes that're "parking
    // blackholes" -- if there are no free spots on that lane, then the roads force cars to a
    // border.
    // TODO Making driving_connection do this.
    pub fn find_driving_lane_near_building(&self, b: BuildingID) -> LaneID {
        if let Ok(l) = self.find_closest_lane(self.get_b(b).sidewalk(), vec![LaneType::Driving]) {
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

            for (lane, lane_type) in r
                .children_forwards
                .iter()
                .chain(r.children_backwards.iter())
            {
                if *lane_type == LaneType::Driving {
                    if !self.get_l(*lane).driving_blackhole {
                        return *lane;
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

    pub fn pathfind(&self, req: PathRequest) -> Option<Path> {
        assert!(!self.pathfinder_dirty);
        self.pathfinder.as_ref().unwrap().pathfind(req, self)
    }

    pub fn should_use_transit(
        &self,
        start: Position,
        end: Position,
    ) -> Option<(BusStopID, Option<BusStopID>, BusRouteID)> {
        self.pathfinder
            .as_ref()
            .unwrap()
            .should_use_transit(self, start, end)
    }

    // None for SharedSidewalkCorners
    pub fn get_turn_group(&self, t: TurnID) -> Option<TurnGroupID> {
        if let Some(ref ts) = self.maybe_get_traffic_signal(t.parent) {
            if self.get_t(t).turn_type == TurnType::SharedSidewalkCorner {
                return None;
            }
            for tg in ts.turn_groups.values() {
                if tg.members.contains(&t) {
                    return Some(tg.id);
                }
            }
            panic!("{} doesn't belong to any turn groups", t);
        }
        None
    }

    pub fn find_r_by_osm_id(
        &self,
        osm_way_id: i64,
        osm_node_ids: (i64, i64),
    ) -> Result<RoadID, String> {
        for r in self.all_roads() {
            if r.orig_id.osm_way_id == osm_way_id
                && r.orig_id.i1.osm_node_id == osm_node_ids.0
                && r.orig_id.i2.osm_node_id == osm_node_ids.1
            {
                return Ok(r.id);
            }
        }
        Err(format!(
            "Can't find osm_way_id {} between nodes {} and {}",
            osm_way_id, osm_node_ids.0, osm_node_ids.1
        ))
    }

    // TODO Take OriginalIntersection
    pub fn find_i_by_osm_id(&self, osm_node_id: i64) -> Result<IntersectionID, String> {
        for i in self.all_intersections() {
            if i.orig_id.osm_node_id == osm_node_id {
                return Ok(i.id);
            }
        }
        Err(format!("Can't find osm_node_id {}", osm_node_id))
    }

    pub fn find_b_by_osm_id(&self, osm_way_id: i64) -> Option<BuildingID> {
        for b in self.all_buildings() {
            if b.osm_way_id == osm_way_id {
                return Some(b.id);
            }
        }
        None
    }

    pub fn right_shift(&self, pl: PolyLine, width: Distance) -> PolyLine {
        self.config.driving_side.right_shift(pl, width)
    }
    pub fn left_shift(&self, pl: PolyLine, width: Distance) -> PolyLine {
        self.config.driving_side.left_shift(pl, width)
    }
    pub fn right_shift_line(&self, line: Line, width: Distance) -> Line {
        self.config.driving_side.right_shift_line(line, width)
    }
    pub fn left_shift_line(&self, line: Line, width: Distance) -> Line {
        self.config.driving_side.left_shift_line(line, width)
    }
    pub fn driving_side_angle(&self, a: Angle) -> Angle {
        self.config.driving_side.angle_offset(a)
    }
    // Last resort
    pub fn get_driving_side(&self) -> DrivingSide {
        self.config.driving_side
    }

    // TODO Sort of a temporary hack
    pub fn hack_override_offstreet_spots(&mut self, spots_per_bldg: usize) {
        for b in &mut self.buildings {
            if let OffstreetParking::Private(ref mut num_spots) = b.parking {
                *num_spots = spots_per_bldg;
            }
        }
    }
    pub fn hack_override_offstreet_spots_individ(&mut self, b: BuildingID, spots: usize) {
        let b = &mut self.buildings[b.0];
        if let OffstreetParking::Private(ref mut num_spots) = b.parking {
            *num_spots = spots;
        }
    }

    pub fn hack_override_bldg_type(&mut self, b: BuildingID, bldg_type: BuildingType) {
        self.buildings[b.0].bldg_type = bldg_type;
    }
}
