// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use abstutil;
use abstutil::Error;
use edits::RoadEdits;
use flame;
use geom::{Bounds, HashablePt2D, PolyLine, Pt2D};
use make;
use raw_data;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io;
use std::path;
use {
    Area, AreaID, Building, BuildingID, BusRoute, BusStop, BusStopID, Intersection, IntersectionID,
    Lane, LaneID, LaneType, Parcel, ParcelID, Road, RoadID, Turn, TurnID, LANE_THICKNESS,
};

#[derive(Serialize, Deserialize, Debug)]
pub struct Map {
    roads: Vec<Road>,
    lanes: Vec<Lane>,
    intersections: Vec<Intersection>,
    turns: BTreeMap<TurnID, Turn>,
    buildings: Vec<Building>,
    parcels: Vec<Parcel>,
    bus_stops: BTreeMap<BusStopID, BusStop>,
    bus_routes: Vec<BusRoute>,
    areas: Vec<Area>,

    // TODO maybe dont need to retain GPS stuff later
    bounds: Bounds,

    name: String,
    road_edits: RoadEdits,
}

impl Map {
    pub fn new(path: &str, road_edits: RoadEdits) -> Result<Map, io::Error> {
        // TODO I think I want something a bit different than flame:
        // - Print as each phase occurs
        // - Print with nicely formatted durations
        flame::start("read raw_data");
        let data: raw_data::Map = abstutil::read_binary(path)?;
        flame::end("read raw_data");
        Ok(Map::create_from_raw(
            path::Path::new(path)
                .file_stem()
                .unwrap()
                .to_os_string()
                .into_string()
                .unwrap(),
            data,
            road_edits,
        ))
    }

    pub fn create_from_raw(name: String, data: raw_data::Map, road_edits: RoadEdits) -> Map {
        let bounds = data.get_gps_bounds();
        let mut m = Map {
            name,
            road_edits,
            bounds,
            roads: Vec::new(),
            lanes: Vec::new(),
            intersections: Vec::new(),
            turns: BTreeMap::new(),
            buildings: Vec::new(),
            parcels: Vec::new(),
            bus_stops: BTreeMap::new(),
            bus_routes: Vec::new(),
            areas: Vec::new(),
        };

        let mut pt_to_intersection: HashMap<HashablePt2D, IntersectionID> = HashMap::new();

        for (idx, i) in data.intersections.iter().enumerate() {
            let id = IntersectionID(idx);
            let pt = Pt2D::from_gps(&i.point, &bounds);
            m.intersections.push(Intersection {
                id,
                point: pt,
                turns: Vec::new(),
                elevation: i.elevation,
                has_traffic_signal: i.has_traffic_signal,
                incoming_lanes: Vec::new(),
                outgoing_lanes: Vec::new(),
            });
            pt_to_intersection.insert(HashablePt2D::from(pt), id);
        }

        let mut counter = 0;
        for (idx, r) in data.roads.iter().enumerate() {
            let road_id = RoadID(idx);
            let road_center_pts = PolyLine::new(
                r.points
                    .iter()
                    .map(|coord| Pt2D::from_gps(coord, &bounds))
                    .collect(),
            );

            m.roads.push(Road {
                id: road_id,
                osm_tags: r.osm_tags.clone(),
                osm_way_id: r.osm_way_id,
                children_forwards: Vec::new(),
                children_backwards: Vec::new(),
                center_pts: road_center_pts.clone(),
            });

            let i1 = pt_to_intersection[&HashablePt2D::from(road_center_pts.first_pt())];
            let i2 = pt_to_intersection[&HashablePt2D::from(road_center_pts.last_pt())];

            // TODO move this to make/lanes.rs too
            for lane in make::get_lane_specs(r, road_id, &m.road_edits) {
                let id = LaneID(counter);
                counter += 1;

                let mut unshifted_pts = road_center_pts.clone();
                if lane.reverse_pts {
                    unshifted_pts = unshifted_pts.reversed();
                }
                let (src_i, dst_i) = if lane.reverse_pts { (i2, i1) } else { (i1, i2) };
                m.intersections[src_i.0].outgoing_lanes.push(id);
                m.intersections[dst_i.0].incoming_lanes.push(id);

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

        for i in &m.intersections {
            make::trim_lines(&mut m.lanes, i);
            if i.incoming_lanes.is_empty() && i.outgoing_lanes.is_empty() {
                panic!("{:?} is orphaned!", i);
            }
        }

        let (stops, routes) =
            make::make_bus_stops(&mut m.lanes, &m.roads, &data.bus_routes, &bounds);
        m.bus_stops = stops;

        for i in &m.intersections {
            for t in make::make_all_turns(i, &m) {
                assert!(!m.turns.contains_key(&t.id));
                m.turns.insert(t.id, t);
            }
        }
        for t in m.turns.values() {
            m.intersections[t.parent.0].turns.push(t.id);
        }

        {
            let _guard = flame::start_guard(format!("make {} buildings", data.buildings.len()));
            make::make_all_buildings(&mut m.buildings, &data.buildings, &bounds, &m.lanes);
            for b in &m.buildings {
                m.lanes[b.front_path.sidewalk.0].building_paths.push(b.id);
            }
        }

        {
            let _guard = flame::start_guard(format!("make {} parcels", data.parcels.len()));
            make::make_all_parcels(&mut m.parcels, &data.parcels, &bounds, &m.lanes);
        }

        for (idx, a) in data.areas.iter().enumerate() {
            m.areas.push(Area {
                id: AreaID(idx),
                area_type: a.area_type,
                points: a
                    .points
                    .iter()
                    .map(|coord| Pt2D::from_gps(coord, &bounds))
                    .collect(),
                osm_tags: a.osm_tags.clone(),
                osm_way_id: a.osm_way_id,
            });
        }

        {
            let _guard = flame::start_guard(format!("verify {} bus routes", routes.len()));
            m.bus_routes = make::verify_bus_routes(&m, routes);
        }

        m
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

    // The turns may belong to two different intersections!
    pub fn get_turns_from_lane(&self, l: LaneID) -> Vec<&Turn> {
        let lane = self.get_l(l);
        let mut turns: Vec<&Turn> = self
            .get_i(lane.dst_i)
            .turns
            .iter()
            .map(|t| self.get_t(*t))
            .filter(|t| t.src == l)
            .collect();
        // Sidewalks are bidirectional
        if lane.is_sidewalk() {
            for t in &self.get_i(lane.src_i).turns {
                let turn = self.get_t(*t);
                if turn.src == l {
                    turns.push(turn);
                }
            }
        }
        turns
    }

    pub fn get_next_lanes(&self, from: LaneID) -> Vec<&Lane> {
        // TODO assumes no duplicates
        self.get_turns_from_lane(from)
            .iter()
            .map(|t| self.get_l(t.dst))
            .collect()
    }

    pub fn get_parent(&self, id: LaneID) -> &Road {
        let l = self.get_l(id);
        self.get_r(l.parent)
    }

    // TODO can we return a borrow?
    pub fn get_gps_bounds(&self) -> Bounds {
        self.bounds.clone()
    }

    pub fn get_driving_lane_from_bldg(&self, bldg: BuildingID) -> Result<LaneID, Error> {
        let sidewalk = self.get_b(bldg).front_path.sidewalk;
        let road = self.get_parent(sidewalk);
        road.find_driving_lane_from_sidewalk(sidewalk)
    }

    pub fn get_sidewalk_from_driving_lane(&self, driving: LaneID) -> Result<LaneID, Error> {
        let road = self.get_parent(driving);
        // No parking lane?
        if let Ok(l) = road.find_sidewalk(driving) {
            return Ok(l);
        }
        road.find_parking_lane(driving)
            .and_then(|parking| road.find_sidewalk(parking))
            .map_err(|e| e.context(format!("get_sidewalk_from_driving_lane({})", driving)))
    }

    pub fn get_driving_lane_from_parking(&self, parking: LaneID) -> Result<LaneID, Error> {
        self.get_parent(parking).find_driving_lane(parking)
    }

    pub fn get_name(&self) -> &String {
        &self.name
    }

    pub fn get_road_edits(&self) -> &RoadEdits {
        &self.road_edits
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
    pub fn get_connected_bus_stops(&self, start: BusStopID) -> BTreeSet<BusStopID> {
        let mut stops: BTreeSet<BusStopID> = BTreeSet::new();
        for r in &self.bus_routes {
            if r.stops.contains(&start) {
                stops.extend(r.stops.clone());
            }
        }
        stops.remove(&start);
        stops
    }
}
