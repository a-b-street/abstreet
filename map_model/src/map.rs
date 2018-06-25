// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use Bounds;
use Pt2D;
use building;
use building::{Building, BuildingID};
use dimensioned::si;
use geometry;
use get_gps_bounds;
use intersection::{Intersection, IntersectionID};
use parcel::{Parcel, ParcelID};
use pb;
use road;
use road::{LaneType, Road, RoadID};
use std::collections::HashMap;
use turn::{Turn, TurnID};

pub struct Map {
    roads: Vec<Road>,
    intersections: Vec<Intersection>,
    turns: Vec<Turn>,
    buildings: Vec<Building>,
    parcels: Vec<Parcel>,

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
            bounds: get_gps_bounds(data),
        };
        let bounds = m.get_gps_bounds();

        let mut pt_to_intersection: HashMap<Pt2D, IntersectionID> = HashMap::new();

        for (idx, i) in data.get_intersections().iter().enumerate() {
            let id = IntersectionID(idx);
            let pt = geometry::gps_to_screen_space(&Pt2D::from(i.get_point()), &bounds);
            m.intersections.push(Intersection {
                id,
                point: pt,
                turns: Vec::new(),
                elevation_meters: i.get_elevation_meters(),
                // TODO use the data again!
                //has_traffic_signal: i.get_has_traffic_signal(),
                has_traffic_signal: idx % 2 == 0,
                incoming_roads: Vec::new(),
                outgoing_roads: Vec::new(),
            });
            pt_to_intersection.insert(pt, id);
        }

        let mut counter = 0;
        for r in data.get_roads() {
            let oneway = r.get_osm_tags().contains(&String::from("oneway=yes"));
            // These seem to represent weird roundabouts
            let junction = r.get_osm_tags().contains(&String::from("junction=yes"));

            let orig_direction = true;
            let reverse_direction = false;
            // lane_type, offset, reverse the points or not, offset to get the other_side's ID
            let mut lanes: Vec<(LaneType, u8, bool, Option<isize>)> = vec![
                (
                    LaneType::Driving,
                    0,
                    orig_direction,
                    if oneway || junction { None } else { Some(3) },
                ),
                (LaneType::Parking, 1, orig_direction, None),
                (LaneType::Sidewalk, 2, orig_direction, None),
            ];
            if junction {
                lanes.pop();
                lanes.pop();
            } else if oneway {
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

                let mut unshifted_pts: Vec<Pt2D> = r.get_points()
                    .iter()
                    .map(|coord| geometry::gps_to_screen_space(&Pt2D::from(coord), &bounds))
                    .collect();
                if lane.2 != orig_direction {
                    unshifted_pts.reverse();
                }

                // Do this with the original points, before trimming them back
                let i1 = pt_to_intersection[&unshifted_pts[0]];
                let i2 = pt_to_intersection[unshifted_pts.last().unwrap()];
                m.intersections[i1.0].outgoing_roads.push(id);
                m.intersections[i2.0].incoming_roads.push(id);

                let offset = lane.1;
                let use_yellow_center_lines = if let Some(other) = other_side {
                    id.0 < other.0
                } else {
                    lane.1 == 0
                };
                let lane_center_lines = road::calculate_lane_center_lines(
                    &unshifted_pts,
                    offset,
                    use_yellow_center_lines,
                );

                // pts and lane_center_lines will get updated in the next pass
                m.roads.push(Road {
                    id,
                    other_side,
                    offset,
                    use_yellow_center_lines,
                    lane_center_lines,
                    unshifted_pts,
                    src_i: i1,
                    dst_i: i2,
                    osm_tags: r.get_osm_tags().to_vec(),
                    osm_way_id: r.get_osm_way_id(),
                    lane_type: lane.0,
                });
            }
        }

        for i in &m.intersections {
            trim_lines(&mut m.roads, i);
        }

        for i in &m.intersections {
            let turns = make_turns(i, &m);
            m.turns.extend(turns);
        }
        for t in &m.turns {
            m.intersections[t.parent.0].turns.push(t.id);
        }

        for (idx, b) in data.get_buildings().iter().enumerate() {
            let points = b.get_points()
                .iter()
                .map(|coord| geometry::gps_to_screen_space(&Pt2D::from(coord), &bounds))
                .collect();
            let osm_tags = b.get_osm_tags().to_vec();
            let front_path = building::find_front_path(&points, &osm_tags, &m);

            m.buildings.push(Building {
                points,
                osm_tags,
                front_path,
                id: BuildingID(idx),
                osm_way_id: b.get_osm_way_id(),
            });
        }

        for (idx, p) in data.get_parcels().iter().enumerate() {
            m.parcels.push(Parcel {
                id: ParcelID(idx),
                points: p.get_points()
                    .iter()
                    .map(|coord| geometry::gps_to_screen_space(&Pt2D::from(coord), &bounds))
                    .collect(),
            });
        }

        m
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
        self.get_i(self.get_r(r).src_i)
    }

    pub fn get_destination_intersection(&self, r: RoadID) -> &Intersection {
        self.get_i(self.get_r(r).dst_i)
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

// TODO organize these differently
fn make_turns(i: &Intersection, m: &Map) -> Vec<Turn> {
    let incoming: Vec<RoadID> = i.incoming_roads
        .iter()
        .filter(|id| m.roads[id.0].lane_type == LaneType::Driving)
        .map(|id| *id)
        .collect();
    let outgoing: Vec<RoadID> = i.outgoing_roads
        .iter()
        .filter(|id| m.roads[id.0].lane_type == LaneType::Driving)
        .map(|id| *id)
        .collect();

    // TODO: Figure out why this happens in the huge map
    if incoming.is_empty() {
        println!("WARNING: intersection {:?} has no incoming roads", i);
        return Vec::new();
    }
    if outgoing.is_empty() {
        println!("WARNING: intersection {:?} has no outgoing roads", i);
        return Vec::new();
    }
    let dead_end = incoming.len() == 1 && outgoing.len() == 1;

    let mut result = Vec::new();
    for src in &incoming {
        let src_r = &m.roads[src.0];
        for dst in &outgoing {
            let dst_r = &m.roads[dst.0];
            // Don't create U-turns unless it's a dead-end
            if src_r.other_side == Some(dst_r.id) && !dead_end {
                continue;
            }

            let id = TurnID(m.turns.len());
            result.push(Turn {
                id,
                parent: i.id,
                src: *src,
                dst: *dst,
                src_pt: src_r.last_pt(),
                dst_pt: dst_r.first_pt(),
            });
        }
    }
    result
}

fn trim_lines(roads: &mut Vec<Road>, i: &Intersection) {
    use std::collections::hash_map::Entry;

    let mut shortest_first_line: HashMap<RoadID, (Pt2D, Pt2D, si::Meter<f64>)> = HashMap::new();
    let mut shortest_last_line: HashMap<RoadID, (Pt2D, Pt2D, si::Meter<f64>)> = HashMap::new();

    fn update_shortest(
        m: &mut HashMap<RoadID, (Pt2D, Pt2D, si::Meter<f64>)>,
        r: RoadID,
        l: (Pt2D, Pt2D),
    ) {
        let new_len = geometry::euclid_dist(l);

        match m.entry(r) {
            Entry::Occupied(mut o) => {
                if new_len < o.get().2 {
                    o.insert((l.0, l.1, new_len));
                }
            }
            Entry::Vacant(v) => {
                v.insert((l.0, l.1, new_len));
            }
        }
    }

    for incoming in &i.incoming_roads {
        for outgoing in &i.outgoing_roads {
            let l1 = *(roads[incoming.0].lane_center_lines.last().unwrap());
            let l2 = roads[outgoing.0].lane_center_lines[0];
            if let Some(hit) = geometry::line_segment_intersection(l1, l2) {
                update_shortest(&mut shortest_last_line, *incoming, (l1.0, hit));
                update_shortest(&mut shortest_first_line, *outgoing, (hit, l2.1));
            }
        }
    }

    // Apply the updates
    /*for (id, triple) in &shortest_first_line {
        roads[id.0].lane_center_lines[0] = (triple.0, triple.1);
    }
    for (id, triple) in &shortest_last_line {
        let len = roads[id.0].lane_center_lines.len();
        roads[id.0].lane_center_lines[len - 1] = (triple.0, triple.1);
    }*/
}
