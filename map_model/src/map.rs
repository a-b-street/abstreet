// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use abstutil;
use building::{Building, BuildingID};
use dimensioned::si;
use edits::Edits;
use geom::{Bounds, HashablePt2D, PolyLine, Pt2D};
use geometry;
use intersection::{Intersection, IntersectionID};
use lane::{Lane, LaneID, LaneType};
use make;
use parcel::{Parcel, ParcelID};
use raw_data;
use road::{Road, RoadID};
use std::collections::HashMap;
use std::io::Error;
use turn::{Turn, TurnID};

pub struct Map {
    roads: Vec<Road>,
    lanes: Vec<Lane>,
    intersections: Vec<Intersection>,
    turns: Vec<Turn>,
    buildings: Vec<Building>,
    parcels: Vec<Parcel>,

    // TODO maybe dont need to retain GPS stuff later
    bounds: Bounds,
}

impl Map {
    pub fn new(path: &str, edits: &Edits) -> Result<Map, Error> {
        let data: raw_data::Map = abstutil::read_binary(path)?;

        let bounds = data.get_gps_bounds();
        let mut m = Map {
            bounds,
            roads: Vec::new(),
            lanes: Vec::new(),
            intersections: Vec::new(),
            turns: Vec::new(),
            buildings: Vec::new(),
            parcels: Vec::new(),
        };

        let mut pt_to_intersection: HashMap<HashablePt2D, IntersectionID> = HashMap::new();

        for (idx, i) in data.intersections.iter().enumerate() {
            let id = IntersectionID(idx);
            let pt = Pt2D::from_gps(&i.point, &bounds);
            m.intersections.push(Intersection {
                id,
                point: pt,
                turns: Vec::new(),
                elevation: i.elevation_meters * si::M,
                has_traffic_signal: i.has_traffic_signal,
                incoming_roads: Vec::new(),
                outgoing_roads: Vec::new(),
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
            m.intersections[i1.0].outgoing_roads.push(road_id);
            m.intersections[i2.0].incoming_roads.push(road_id);

            // TODO move this to make/lanes.rs too
            for lane in make::get_lane_specs(r, road_id, edits) {
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
                let width = geometry::LANE_THICKNESS * ((lane.offset as f64) + 0.5);
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

        for i in &m.intersections {
            let len = m.turns.len();
            let turns = make::make_all_turns(i, &m, len);
            m.turns.extend(turns);
        }
        for t in &m.turns {
            m.intersections[t.parent.0].turns.push(t.id);
        }

        for (idx, b) in data.buildings.iter().enumerate() {
            m.buildings.push(make::make_building(
                b,
                BuildingID(idx),
                &bounds,
                &m.lanes,
                &m.roads,
            ));
        }

        for (idx, p) in data.parcels.iter().enumerate() {
            m.parcels.push(Parcel {
                id: ParcelID(idx),
                points: p.points
                    .iter()
                    .map(|coord| Pt2D::from_gps(coord, &bounds))
                    .collect(),
            });
        }

        Ok(m)
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

    pub fn all_turns(&self) -> &Vec<Turn> {
        &self.turns
    }

    pub fn all_buildings(&self) -> &Vec<Building> {
        &self.buildings
    }

    pub fn all_parcels(&self) -> &Vec<Parcel> {
        &self.parcels
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
        &self.turns[id.0]
    }

    pub fn get_b(&self, id: BuildingID) -> &Building {
        &self.buildings[id.0]
    }

    pub fn get_p(&self, id: ParcelID) -> &Parcel {
        &self.parcels[id.0]
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
        let mut turns: Vec<&Turn> = self.get_i(lane.dst_i)
            .turns
            .iter()
            .map(|t| self.get_t(*t))
            .filter(|t| t.src == l)
            .collect();
        // Sidewalks are bidirectional
        if lane.lane_type == LaneType::Sidewalk {
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

    // TODO can we return a borrow?
    pub fn get_gps_bounds(&self) -> Bounds {
        self.bounds.clone()
    }
}
