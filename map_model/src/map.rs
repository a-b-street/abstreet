use crate::{
    make, raw_data, Area, AreaID, Building, BuildingID, BusRoute, BusRouteID, BusStop, BusStopID,
    ControlStopSign, ControlTrafficSignal, Intersection, IntersectionID, IntersectionType, Lane,
    LaneID, LaneType, MapEdits, Parcel, ParcelID, Road, RoadID, Turn, TurnID, TurnPriority,
};
use abstutil;
use abstutil::{deserialize_btreemap, serialize_btreemap, Error, Timer};
use dimensioned::si;
use geom::{Bounds, GPSBounds, Pt2D};
use serde_derive::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path;

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

        let gps_bounds = data.get_gps_bounds();
        let bounds = gps_bounds.to_bounds();

        make::old_merge_intersections(&mut data, timer);

        let half_map = make::make_half_map(&data, &gps_bounds, &edits, timer);
        let mut m = Map {
            name,
            edits,
            gps_bounds: gps_bounds.clone(),
            bounds: bounds.clone(),
            roads: half_map.roads,
            lanes: half_map.lanes,
            intersections: half_map.intersections,
            turns: half_map.turns,
            buildings: Vec::new(),
            parcels: Vec::new(),
            bus_stops: BTreeMap::new(),
            bus_routes: Vec::new(),
            areas: Vec::new(),
            stop_signs: BTreeMap::new(),
            traffic_signals: BTreeMap::new(),
            turn_lookup: Vec::new(),
        };
        for t in m.turns.values_mut() {
            t.lookup_idx = m.turn_lookup.len();
            m.turn_lookup.push(t.id);
            if t.geom.length() < 0.01 * si::M {
                warn!("u{} is a very short turn", t.lookup_idx);
            }
        }

        let (stops, routes) =
            make::make_bus_stops(&m, &data.bus_routes, &gps_bounds, &bounds, timer);
        m.bus_stops = stops;
        // The IDs are sorted in the BTreeMap, so this order winds up correct.
        for id in m.bus_stops.keys() {
            m.lanes[id.sidewalk.0].bus_stops.push(*id);
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

            // TODO Actually recalculate. This got complicated after merging intersections.
        }
    }

    pub fn edit_stop_sign(&mut self, mut sign: ControlStopSign) {
        sign.changed = true;
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

    pub(crate) fn is_turn_allowed(&self, t: TurnID) -> bool {
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
}
