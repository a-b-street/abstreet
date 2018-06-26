// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use Bounds;
use Pt2D;
use abstutil;
use building;
use building::{Building, BuildingID};
use dimensioned::si;
use geometry;
use intersection::{Intersection, IntersectionID};
use parcel::{Parcel, ParcelID};
use raw_data;
use road::{LaneType, Road, RoadID};
use shift_polyline;
use std::collections::HashMap;
use std::io::Error;
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
    pub fn new(path: &str) -> Result<Map, Error> {
        let data: raw_data::Map = abstutil::read_json(path)?;

        let bounds = data.get_gps_bounds();
        let mut m = Map {
            bounds,
            roads: Vec::new(),
            intersections: Vec::new(),
            turns: Vec::new(),
            buildings: Vec::new(),
            parcels: Vec::new(),
        };

        let mut pt_to_intersection: HashMap<Pt2D, IntersectionID> = HashMap::new();

        for (idx, i) in data.intersections.iter().enumerate() {
            let id = IntersectionID(idx);
            let pt = geometry::gps_to_screen_space(&Pt2D::from(&i.point), &bounds);
            m.intersections.push(Intersection {
                id,
                point: pt,
                turns: Vec::new(),
                elevation: i.elevation_meters * si::M,
                // TODO use the data again!
                //has_traffic_signal: i.get_has_traffic_signal(),
                has_traffic_signal: idx % 2 == 0,
                incoming_roads: Vec::new(),
                outgoing_roads: Vec::new(),
            });
            pt_to_intersection.insert(pt, id);
        }

        let mut counter = 0;
        for r in &data.roads {
            for lane in get_lane_specs(r) {
                let id = RoadID(counter);
                counter += 1;
                let other_side = lane.offset_for_other_id
                    .map(|offset| RoadID(((id.0 as isize) + offset) as usize));

                let mut unshifted_pts: Vec<Pt2D> = r.points
                    .iter()
                    .map(|coord| geometry::gps_to_screen_space(&Pt2D::from(coord), &bounds))
                    .collect();
                if lane.reverse_pts {
                    unshifted_pts.reverse();
                }

                // Do this with the original points, before trimming them back
                let i1 = pt_to_intersection[&unshifted_pts[0]];
                let i2 = pt_to_intersection[unshifted_pts.last().unwrap()];
                m.intersections[i1.0].outgoing_roads.push(id);
                m.intersections[i2.0].incoming_roads.push(id);

                let use_yellow_center_lines = if let Some(other) = other_side {
                    id.0 < other.0
                } else {
                    lane.offset == 0
                };
                // TODO probably different behavior for oneways
                // TODO need to factor in yellow center lines (but what's the right thing to even do?
                // Reverse points for British-style driving on the left
                let lane_center_pts = shift_polyline(
                    geometry::LANE_THICKNESS * ((lane.offset as f64) + 0.5),
                    &unshifted_pts,
                );

                // lane_center_pts will get updated in the next pass
                m.roads.push(Road {
                    id,
                    other_side,
                    use_yellow_center_lines,
                    lane_center_pts,
                    unshifted_pts,
                    offset: lane.offset,
                    src_i: i1,
                    dst_i: i2,
                    osm_tags: r.osm_tags.clone(),
                    osm_way_id: r.osm_way_id,
                    lane_type: lane.lane_type,
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

        // TODO consume data, so we dont have to clone tags?
        for (idx, b) in data.buildings.iter().enumerate() {
            let points = b.points
                .iter()
                .map(|coord| geometry::gps_to_screen_space(&Pt2D::from(coord), &bounds))
                .collect();
            let front_path = building::find_front_path(&points, &b.osm_tags, &m);

            m.buildings.push(Building {
                points,
                front_path,
                id: BuildingID(idx),
                osm_way_id: b.osm_way_id,
                osm_tags: b.osm_tags.clone(),
            });
        }

        for (idx, p) in data.parcels.iter().enumerate() {
            m.parcels.push(Parcel {
                id: ParcelID(idx),
                points: p.points
                    .iter()
                    .map(|coord| geometry::gps_to_screen_space(&Pt2D::from(coord), &bounds))
                    .collect(),
            });
        }

        Ok(m)
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

            let id = TurnID(m.turns.len() + result.len());
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

    // For short first/last lines, this might not work well
    for incoming in &i.incoming_roads {
        for outgoing in &i.outgoing_roads {
            let l1 = roads[incoming.0].last_line();
            let l2 = roads[outgoing.0].first_line();
            if let Some(hit) = geometry::line_segment_intersection(l1, l2) {
                update_shortest(&mut shortest_last_line, *incoming, (l1.0, hit));
                update_shortest(&mut shortest_first_line, *outgoing, (hit, l2.1));
            }
        }
    }

    // Apply the updates
    for (id, triple) in &shortest_first_line {
        roads[id.0].lane_center_pts[0] = triple.0;
        roads[id.0].lane_center_pts[1] = triple.1;
    }
    for (id, triple) in &shortest_last_line {
        let len = roads[id.0].lane_center_pts.len();
        roads[id.0].lane_center_pts[len - 2] = triple.0;
        roads[id.0].lane_center_pts[len - 1] = triple.1;
    }
}

struct LaneSpec {
    lane_type: LaneType,
    offset: u8,
    reverse_pts: bool,
    offset_for_other_id: Option<isize>,
}

fn get_lane_specs(r: &raw_data::Road) -> Vec<LaneSpec> {
    let oneway = r.osm_tags.get("oneway") == Some(&"yes".to_string());
    // These seem to represent weird roundabouts
    let junction = r.osm_tags.get("junction") == Some(&"yes".to_string());

    // TODO debugging convenience
    let only_roads_for_debugging = false;

    let mut lanes: Vec<LaneSpec> = vec![
        LaneSpec {
            lane_type: LaneType::Driving,
            offset: 0,
            reverse_pts: false,
            offset_for_other_id: if oneway || junction {
                None
            } else {
                Some(if only_roads_for_debugging { 1 } else { 3 })
            },
        },
        LaneSpec {
            lane_type: LaneType::Parking,
            offset: 1,
            reverse_pts: false,
            offset_for_other_id: None,
        },
        LaneSpec {
            lane_type: LaneType::Sidewalk,
            offset: 2,
            reverse_pts: false,
            offset_for_other_id: None,
        },
    ];
    if only_roads_for_debugging {
        lanes.pop();
        lanes.pop();
        if !oneway {
            lanes.push(LaneSpec {
                lane_type: LaneType::Driving,
                offset: 0,
                reverse_pts: true,
                offset_for_other_id: Some(-1),
            });
        }
    } else if junction {
        lanes.pop();
        lanes.pop();
    } else if oneway {
        lanes.push(LaneSpec {
            lane_type: LaneType::Sidewalk,
            offset: 0,
            reverse_pts: true,
            offset_for_other_id: None,
        });
    } else {
        lanes.extend(vec![
            LaneSpec {
                lane_type: LaneType::Driving,
                offset: 0,
                reverse_pts: true,
                offset_for_other_id: if oneway || junction {
                    None
                } else {
                    Some(if only_roads_for_debugging { -1 } else { -3 })
                },
            },
            LaneSpec {
                lane_type: LaneType::Parking,
                offset: 1,
                reverse_pts: true,
                offset_for_other_id: None,
            },
            LaneSpec {
                lane_type: LaneType::Sidewalk,
                offset: 2,
                reverse_pts: true,
                offset_for_other_id: None,
            },
        ]);
    }
    lanes
}
