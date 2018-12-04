// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use abstutil;
use abstutil::{deserialize_btreemap, serialize_btreemap, Error, Timer};
use edits::MapEdits;
use geom::{Bounds, GPSBounds, HashablePt2D, PolyLine, Pt2D};
use make;
use raw_data;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io;
use std::path;
use {
    Area, AreaID, Building, BuildingID, BusRoute, BusRouteID, BusStop, BusStopID, ControlStopSign,
    ControlTrafficSignal, Intersection, IntersectionID, IntersectionType, Lane, LaneID, LaneType,
    Parcel, ParcelID, Road, RoadID, Turn, TurnID, LANE_THICKNESS,
};

#[derive(Serialize, Deserialize, Debug)]
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
    parcels: Vec<Parcel>,
    #[serde(
        serialize_with = "serialize_btreemap",
        deserialize_with = "deserialize_btreemap"
    )]
    bus_stops: BTreeMap<BusStopID, BusStop>,
    bus_routes: Vec<BusRoute>,
    areas: Vec<Area>,

    stop_signs: BTreeMap<IntersectionID, ControlStopSign>,
    traffic_signals: BTreeMap<IntersectionID, ControlTrafficSignal>,
    // Note that border nodes belong in neither!
    gps_bounds: GPSBounds,
    bounds: Bounds,

    turn_lookup: Vec<TurnID>,

    name: String,
    edits: MapEdits,
}

impl Map {
    pub fn new(path: &str, edits: MapEdits, timer: &mut Timer) -> Result<Map, io::Error> {
        let data: raw_data::Map = abstutil::read_binary(path, timer)?;
        Ok(Map::create_from_raw(
            path::Path::new(path)
                .file_stem()
                .unwrap()
                .to_os_string()
                .into_string()
                .unwrap(),
            data,
            edits,
            timer,
        ))
    }

    pub fn create_from_raw(
        name: String,
        mut data: raw_data::Map,
        edits: MapEdits,
        timer: &mut Timer,
    ) -> Map {
        timer.start("raw_map to Map");

        if false {
            make::merge_intersections(&mut data, timer);
        }

        let gps_bounds = data.get_gps_bounds();
        let bounds = gps_bounds.to_bounds();

        let mut m = Map {
            name,
            edits,
            gps_bounds: gps_bounds.clone(),
            bounds: bounds.clone(),
            roads: Vec::new(),
            lanes: Vec::new(),
            intersections: Vec::new(),
            turns: BTreeMap::new(),
            buildings: Vec::new(),
            parcels: Vec::new(),
            bus_stops: BTreeMap::new(),
            bus_routes: Vec::new(),
            areas: Vec::new(),
            stop_signs: BTreeMap::new(),
            traffic_signals: BTreeMap::new(),
            turn_lookup: Vec::new(),
        };

        let mut pt_to_intersection: HashMap<HashablePt2D, IntersectionID> = HashMap::new();

        for (idx, i) in data.intersections.iter().enumerate() {
            let id = IntersectionID(idx);
            let pt = Pt2D::from_gps(i.point, &gps_bounds).unwrap();
            m.intersections.push(Intersection {
                id,
                point: pt,
                polygon: Vec::new(),
                turns: Vec::new(),
                elevation: i.elevation,
                // Might change later
                intersection_type: if i.has_traffic_signal {
                    IntersectionType::TrafficSignal
                } else {
                    IntersectionType::StopSign
                },
                incoming_lanes: Vec::new(),
                outgoing_lanes: Vec::new(),
                roads: BTreeSet::new(),
            });
            pt_to_intersection.insert(HashablePt2D::from(pt), id);
        }

        let mut counter = 0;
        timer.start_iter("expand roads to lanes", data.roads.len());
        for (_, r) in data.roads.iter().enumerate() {
            timer.next();
            let road_id = RoadID(m.roads.len());
            let road_center_pts = PolyLine::new(
                r.points
                    .iter()
                    .map(|coord| Pt2D::from_gps(*coord, &gps_bounds).unwrap())
                    .collect(),
            );
            let i1 = pt_to_intersection[&HashablePt2D::from(road_center_pts.first_pt())];
            let i2 = pt_to_intersection[&HashablePt2D::from(road_center_pts.last_pt())];

            if i1 == i2 {
                // TODO Cul-de-sacs should be valid, but it really makes pathfinding screwy
                error!(
                    "OSM way {} is a loop on {}, skipping what would've been {}",
                    r.osm_way_id, i1, road_id
                );
                continue;
            }

            m.roads.push(Road {
                id: road_id,
                osm_tags: r.osm_tags.clone(),
                osm_way_id: r.osm_way_id,
                children_forwards: Vec::new(),
                children_backwards: Vec::new(),
                center_pts: road_center_pts.clone(),
                src_i: i1,
                dst_i: i2,
            });

            // TODO move this to make/lanes.rs too
            for lane in make::get_lane_specs(r, road_id, &m.edits) {
                let id = LaneID(counter);
                counter += 1;

                let mut unshifted_pts = road_center_pts.clone();
                if lane.reverse_pts {
                    unshifted_pts = unshifted_pts.reversed();
                }
                let (src_i, dst_i) = if lane.reverse_pts { (i2, i1) } else { (i1, i2) };
                m.intersections[src_i.0].outgoing_lanes.push(id);
                m.intersections[src_i.0].roads.insert(road_id);
                m.intersections[dst_i.0].incoming_lanes.push(id);
                m.intersections[dst_i.0].roads.insert(road_id);

                // TODO probably different behavior for oneways
                // TODO need to factor in yellow center lines (but what's the right thing to even do?
                // Reverse points for British-style driving on the left
                let width = LANE_THICKNESS * ((lane.offset as f64) + 0.5);
                let (lane_center_pts, probably_broken) = match unshifted_pts.shift(width) {
                    Some(pts) => (pts, false),
                    // TODO wasteful to calculate again, but eh
                    None => (unshifted_pts.shift_blindly(width), true),
                };

                // lane_center_pts will get updated in the next pass
                m.lanes.push(Lane {
                    id,
                    lane_center_pts,
                    probably_broken,
                    src_i,
                    dst_i,
                    lane_type: lane.lane_type,
                    parent: road_id,
                    building_paths: Vec::new(),
                    bus_stops: Vec::new(),
                });
                if lane.reverse_pts {
                    m.roads[road_id.0]
                        .children_backwards
                        .push((id, lane.lane_type));
                } else {
                    m.roads[road_id.0]
                        .children_forwards
                        .push((id, lane.lane_type));
                }
            }
        }

        // TODO gathering results and assigning later is super gross mutability pattern
        let mut intersection_polygons: Vec<Vec<Pt2D>> = Vec::new();
        timer.start_iter("find each intersection polygon", m.intersections.len());
        for i in &m.intersections {
            timer.next();

            if i.incoming_lanes.is_empty() && i.outgoing_lanes.is_empty() {
                panic!("{:?} is orphaned!", i);
            }

            intersection_polygons.push(make::intersection_polygon(i, &m.roads));
        }
        for (idx, p) in intersection_polygons.into_iter().enumerate() {
            m.intersections[idx].polygon = p;
        }

        timer.start_iter("trim lanes at each intersection", m.intersections.len());
        for i in &m.intersections {
            timer.next();
            make::trim_lines(&mut m.lanes, i);
        }

        for i in 0..m.intersections.len() {
            // Is the intersection a border?
            if is_border(&m.intersections[i], &m) {
                m.intersections[i].intersection_type = IntersectionType::Border;
            }
        }

        let (stops, routes) =
            make::make_bus_stops(&m, &data.bus_routes, &gps_bounds, &bounds, timer);
        m.bus_stops = stops;
        // The IDs are sorted in the BTreeMap, so this order winds up correct.
        for id in m.bus_stops.keys() {
            m.lanes[id.sidewalk.0].bus_stops.push(*id);
        }

        for i in &m.intersections {
            for t in make::make_all_turns(i, &m) {
                assert!(!m.turns.contains_key(&t.id));
                m.turns.insert(t.id, t);
            }
        }
        for (idx, t) in m.turns.values_mut().enumerate() {
            t.lookup_idx = idx;
        }
        for t in m.turns.values() {
            m.intersections[t.id.parent.0].turns.push(t.id);
            m.turn_lookup.push(t.id);
        }

        let mut stop_signs: BTreeMap<IntersectionID, ControlStopSign> = BTreeMap::new();
        let mut traffic_signals: BTreeMap<IntersectionID, ControlTrafficSignal> = BTreeMap::new();
        for i in &m.intersections {
            match i.intersection_type {
                IntersectionType::StopSign => {
                    stop_signs.insert(i.id, ControlStopSign::new(&m, i.id));
                }
                IntersectionType::TrafficSignal => {
                    traffic_signals.insert(i.id, ControlTrafficSignal::new(&m, i.id));
                }
                IntersectionType::Border => {}
            };
        }
        // Override with edits
        for (i, ss) in &m.edits.stop_signs {
            stop_signs.insert(*i, ss.clone());
        }
        for (i, ts) in &m.edits.traffic_signals {
            traffic_signals.insert(*i, ts.clone());
        }
        m.stop_signs = stop_signs;
        m.traffic_signals = traffic_signals;

        make::make_all_buildings(
            &mut m.buildings,
            &data.buildings,
            &gps_bounds,
            &bounds,
            &m.lanes,
            timer,
        );
        for b in &m.buildings {
            m.lanes[b.front_path.sidewalk.lane().0]
                .building_paths
                .push(b.id);
        }

        make::make_all_parcels(
            &mut m.parcels,
            &data.parcels,
            &gps_bounds,
            &bounds,
            &m.lanes,
            timer,
        );

        for (idx, a) in data.areas.iter().enumerate() {
            m.areas.push(Area {
                id: AreaID(idx),
                area_type: a.area_type,
                points: a
                    .points
                    .iter()
                    .map(|coord| Pt2D::from_gps(*coord, &gps_bounds).unwrap())
                    .collect(),
                osm_tags: a.osm_tags.clone(),
                osm_way_id: a.osm_way_id,
            });
        }

        m.bus_routes = make::verify_bus_routes(&m, routes, timer);

        timer.stop("raw_map to Map");
        m
    }

    // The caller has to clone get_edits(), mutate, actualize the changes, then store them
    // here.
    // TODO Only road editor calls this. Stop sign / traffic signal editor have a nicer pattern.
    pub fn store_new_edits(&mut self, edits: MapEdits) {
        self.edits = edits;
    }

    pub fn edit_lane_type(&mut self, lane: LaneID, new_type: LaneType) {
        assert_ne!(self.get_l(lane).lane_type, new_type);
        self.lanes[lane.0].lane_type = new_type;
        let parent = self.get_l(lane).parent;
        self.roads[parent.0].edit_lane_type(lane, new_type);

        // Recalculate all of the turns at the two connected intersections.
        for i in self.get_l(lane).intersections().into_iter() {
            for t in &self.intersections[i.0].turns {
                self.turns.remove(t);
            }
            self.intersections[i.0].turns.clear();

            for t in make::make_all_turns(self.get_i(i), &self) {
                // TODO ahh need to dedupe
                self.intersections[i.0].turns.push(t.id);
                self.turns.insert(t.id, t);
            }
        }
    }

    pub fn edit_stop_sign(&mut self, sign: ControlStopSign) {
        self.edits.stop_signs.insert(sign.id, sign.clone());
        self.stop_signs.insert(sign.id, sign);
    }

    pub fn edit_traffic_signal(&mut self, mut signal: ControlTrafficSignal) {
        signal.changed = true;
        self.edits.traffic_signals.insert(signal.id, signal.clone());
        self.traffic_signals.insert(signal.id, signal);
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

    pub fn all_parcels(&self) -> &Vec<Parcel> {
        &self.parcels
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

    pub fn maybe_get_p(&self, id: ParcelID) -> Option<&Parcel> {
        self.parcels.get(id.0)
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

    pub fn get_p(&self, id: ParcelID) -> &Parcel {
        &self.parcels[id.0]
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
        self.turn_lookup.get(idx).map(|id| *id)
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

    // TODO Get rid of this, or rewrite it in in terms of get_turns_from_lane_at_end
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

    pub fn get_edits(&self) -> &MapEdits {
        &self.edits
    }

    pub fn all_bus_stops(&self) -> &BTreeMap<BusStopID, BusStop> {
        &self.bus_stops
    }

    pub fn get_bs(&self, stop: BusStopID) -> &BusStop {
        &self.bus_stops[&stop]
    }

    pub fn get_all_bus_routes(&self) -> &Vec<BusRoute> {
        &self.bus_routes
    }

    pub fn get_bus_route(&self, name: &str) -> Option<&BusRoute> {
        self.bus_routes.iter().find(|r| r.name == name)
    }

    // Not including transfers
    pub fn get_connected_bus_stops(&self, start: BusStopID) -> Vec<(BusStopID, BusRouteID)> {
        let mut stops = Vec::new();
        for r in &self.bus_routes {
            if r.stops.contains(&start) {
                for stop in &r.stops {
                    if *stop != start {
                        stops.push((*stop, r.id));
                    }
                }
            }
        }
        stops
    }

    pub fn building_to_road(&self, id: BuildingID) -> &Road {
        self.get_parent(self.get_b(id).front_path.sidewalk.lane())
    }

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
        let path = format!("../data/maps/{}_{}.abst", self.name, self.edits.edits_name);
        info!("Saving {}...", path);
        abstutil::write_binary(&path, self).expect(&format!("Saving {} failed", path));
        info!("Saved {}", path);
    }

    pub fn find_closest_lane(&self, from: LaneID, types: Vec<LaneType>) -> Result<LaneID, Error> {
        self.get_parent(from).find_closest_lane(from, types)
    }

    pub fn find_closest_lane_to_bldg(
        &self,
        bldg: BuildingID,
        types: Vec<LaneType>,
    ) -> Result<LaneID, Error> {
        let from = self.get_b(bldg).front_path.sidewalk.lane();
        self.find_closest_lane(from, types)
    }

    // TODO reconsider names, or put somewhere else?
    pub fn bldg(&self, label: &str) -> BuildingID {
        for b in &self.buildings {
            if b.osm_tags.get("label") == Some(&label.to_string()) {
                return b.id;
            }
        }
        panic!("No building has label {}", label);
    }

    pub fn parking_lane(&self, label: &str, expected_spots: usize) -> LaneID {
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
                return l.id;
            }
        }
        panic!("No parking lane has label {}", label);
    }
}

fn is_border(intersection: &Intersection, map: &Map) -> bool {
    // Bias for driving
    if !intersection.is_dead_end() {
        return false;
    }
    let has_driving_in = intersection
        .incoming_lanes
        .iter()
        .find(|l| map.get_l(**l).is_driving())
        .is_some();
    let has_driving_out = intersection
        .outgoing_lanes
        .iter()
        .find(|l| map.get_l(**l).is_driving())
        .is_some();
    has_driving_in != has_driving_out
}
