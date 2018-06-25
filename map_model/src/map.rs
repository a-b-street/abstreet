// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use Bounds;
use Pt2D;
use building::{Building, BuildingID};
use get_gps_bounds;
use intersection::{Intersection, IntersectionID};
use parcel::{Parcel, ParcelID};
use pb;
use road::{LaneType, Road, RoadID};
use std::collections::HashMap;
use turn::{Turn, TurnID};

pub struct Map {
    roads: Vec<Road>,
    intersections: Vec<Intersection>,
    turns: Vec<Turn>,
    buildings: Vec<Building>,
    parcels: Vec<Parcel>,

    pt_to_intersection: HashMap<Pt2D, IntersectionID>,
    intersection_to_roads: HashMap<IntersectionID, Vec<RoadID>>,

    // TODO maybe dont need to retain GPS stuff later
    bounds: Bounds,
}

impl Map {
    pub fn new(data: &pb::Map) -> Map {
        let mut m = Map {
            roads: Vec::new(),
            intersections: Vec::new(),
            turns: Vec::new(),
            buildings: Vec::new(),
            parcels: Vec::new(),
            pt_to_intersection: HashMap::new(),
            intersection_to_roads: HashMap::new(),
            bounds: get_gps_bounds(data),
        };

        for (idx, i) in data.get_intersections().iter().enumerate() {
            let id = IntersectionID(idx);
            let pt = Pt2D::from(i.get_point());
            m.intersections.push(Intersection {
                id,
                point: pt,
                turns: Vec::new(),
                elevation_meters: i.get_elevation_meters(),
                // TODO use the data again!
                //has_traffic_signal: i.get_has_traffic_signal(),
                has_traffic_signal: idx % 2 == 0,
            });
            m.pt_to_intersection.insert(pt, id);
        }

        let mut counter = 0;
        for r in data.get_roads() {
            let oneway = r.get_osm_tags().contains(&String::from("oneway=yes"));

            let orig_direction = true;
            let reverse_direction = false;
            // lane_type, offset, reverse the points or not, offset to get the other_side's ID
            let mut lanes: Vec<(LaneType, u8, bool, Option<isize>)> = vec![
                (
                    LaneType::Driving,
                    0,
                    orig_direction,
                    if oneway { None } else { Some(3) },
                ),
                (LaneType::Parking, 1, orig_direction, None),
                (LaneType::Sidewalk, 2, orig_direction, None),
            ];
            if oneway {
                lanes.push((LaneType::Sidewalk, 0, reverse_direction, None));
            } else {
                lanes.extend(vec![
                    (LaneType::Driving, 0, reverse_direction, Some(-3)),
                    (LaneType::Parking, 1, reverse_direction, None),
                    (LaneType::Sidewalk, 2, reverse_direction, None),
                ]);
            }

            for lane in &lanes {
                let id = RoadID(counter);
                counter += 1;
                let other_side = lane.3
                    .map(|offset| RoadID(((id.0 as isize) + offset) as usize));

                let pts: Vec<Pt2D> = if lane.2 == orig_direction {
                    r.get_points().iter().map(Pt2D::from).collect()
                } else {
                    r.get_points().iter().rev().map(Pt2D::from).collect()
                };
                let i1 = m.pt_to_intersection[&pts[0]];
                let i2 = m.pt_to_intersection[pts.last().unwrap()];
                m.intersection_to_roads
                    .entry(i1)
                    .or_insert_with(Vec::new)
                    .push(id);
                m.intersection_to_roads
                    .entry(i2)
                    .or_insert_with(Vec::new)
                    .push(id);

                m.roads.push(Road {
                    id,
                    other_side,
                    osm_tags: r.get_osm_tags().to_vec(),
                    osm_way_id: r.get_osm_way_id(),
                    lane_type: lane.0,
                    offset: lane.1,
                    points: pts,
                    use_yellow_center_lines: if let Some(other) = other_side {
                        id.0 < other.0
                    } else {
                        lane.1 == 0
                    },
                });
            }
        }

        for i in &m.intersections {
            // TODO: Figure out why this happens in the huge map
            if m.intersection_to_roads.get(&i.id).is_none() {
                println!("WARNING: intersection {:?} has no roads", i);
                continue;
            }
            let incident_roads: Vec<RoadID> = m.intersection_to_roads[&i.id]
                .iter()
                .filter(|id| m.roads[id.0].lane_type == LaneType::Driving)
                .map(|id| *id)
                .collect();
            for src in &incident_roads {
                let src_r = &m.roads[src.0];
                if i.point != *src_r.points.last().unwrap() {
                    continue;
                }
                for dst in &incident_roads {
                    let dst_r = &m.roads[dst.0];
                    if i.point != dst_r.points[0] {
                        continue;
                    }
                    // Don't create U-turns unless it's a dead-end
                    if src_r.other_side == Some(dst_r.id) && incident_roads.len() > 2 {
                        continue;
                    }

                    let id = TurnID(m.turns.len());
                    m.turns.push(Turn {
                        id,
                        parent: i.id,
                        src: *src,
                        dst: *dst,
                    });
                }
            }
        }
        for t in &m.turns {
            m.intersections[t.parent.0].turns.push(t.id);
        }

        for (idx, b) in data.get_buildings().iter().enumerate() {
            m.buildings.push(Building {
                id: BuildingID(idx),
                points: b.get_points().iter().map(Pt2D::from).collect(),
                osm_tags: b.get_osm_tags().to_vec(),
                osm_way_id: b.get_osm_way_id(),
            });
        }

        for (idx, p) in data.get_parcels().iter().enumerate() {
            m.parcels.push(Parcel {
                id: ParcelID(idx),
                points: p.get_points().iter().map(Pt2D::from).collect(),
            });
        }

        m
    }

    pub fn get_roads_to_intersection(&self, id: IntersectionID) -> Vec<&Road> {
        self.intersection_to_roads[&id]
            .iter()
            .map(|id| &self.roads[id.0])
            .filter(|r| *r.points.last().unwrap() == self.get_i(id).point)
            .collect()
    }

    pub fn get_roads_from_intersection(&self, id: IntersectionID) -> Vec<&Road> {
        self.intersection_to_roads[&id]
            .iter()
            .map(|id| &self.roads[id.0])
            .filter(|r| r.points[0] == self.get_i(id).point)
            .collect()
    }

    pub fn all_roads(&self) -> &Vec<Road> {
        &self.roads
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

    pub fn get_source_intersection(&self, r: RoadID) -> &Intersection {
        self.get_i(self.pt_to_intersection[&self.get_r(r).points[0]])
    }

    pub fn get_destination_intersection(&self, r: RoadID) -> &Intersection {
        self.get_i(self.pt_to_intersection[self.get_r(r).points.last().unwrap()])
    }

    pub fn get_turns_in_intersection(&self, id: IntersectionID) -> Vec<&Turn> {
        self.get_i(id)
            .turns
            .iter()
            .map(|t| self.get_t(*t))
            .collect()
    }

    pub fn get_turns_from_road(&self, id: RoadID) -> Vec<&Turn> {
        let i = self.get_destination_intersection(id);
        // TODO can't filter on get_turns_in_intersection... winds up being Vec<&&Turn>
        i.turns
            .iter()
            .map(|t| self.get_t(*t))
            .filter(|t| t.src == id)
            .collect()
    }

    pub fn get_next_roads(&self, from: RoadID) -> Vec<&Road> {
        // TODO assumes no duplicates
        self.get_turns_from_road(from)
            .iter()
            .map(|t| self.get_r(t.dst))
            .collect()
    }

    // TODO can we return a borrow?
    pub fn get_gps_bounds(&self) -> Bounds {
        self.bounds.clone()
    }
}
