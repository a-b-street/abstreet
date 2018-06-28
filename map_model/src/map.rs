// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use Bounds;
use HashablePt2D;
use Pt2D;
use abstutil;
use building::{Building, BuildingID};
use dimensioned::si;
use geometry;
use intersection::{Intersection, IntersectionID};
use make;
use parcel::{Parcel, ParcelID};
use raw_data;
use road::{Road, RoadID};
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
        let data: raw_data::Map = abstutil::read_binary(path)?;

        let bounds = data.get_gps_bounds();
        let mut m = Map {
            bounds,
            roads: Vec::new(),
            intersections: Vec::new(),
            turns: Vec::new(),
            buildings: Vec::new(),
            parcels: Vec::new(),
        };

        let mut pt_to_intersection: HashMap<HashablePt2D, IntersectionID> = HashMap::new();

        for (idx, i) in data.intersections.iter().enumerate() {
            let id = IntersectionID(idx);
            let pt = geometry::gps_to_screen_space(&i.point, &bounds);
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
            pt_to_intersection.insert(HashablePt2D::from(pt), id);
        }

        let mut counter = 0;
        for r in &data.roads {
            // TODO move this to make/lanes.rs too
            for lane in make::get_lane_specs(r) {
                let id = RoadID(counter);
                counter += 1;
                let other_side = lane.offset_for_other_id
                    .map(|offset| RoadID(((id.0 as isize) + offset) as usize));

                let mut unshifted_pts: Vec<Pt2D> = r.points
                    .iter()
                    .map(|coord| geometry::gps_to_screen_space(&coord, &bounds))
                    .collect();
                if lane.reverse_pts {
                    unshifted_pts.reverse();
                }

                // Do this with the original points, before trimming them back
                let i1 = pt_to_intersection[&HashablePt2D::from(unshifted_pts[0])];
                let i2 = pt_to_intersection[&HashablePt2D::from(*unshifted_pts.last().unwrap())];
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
            make::trim_lines(&mut m.roads, i);
        }

        for i in &m.intersections {
            let turns = make::make_turns(i, &m, m.turns.len());
            m.turns.extend(turns);
        }
        for t in &m.turns {
            m.intersections[t.parent.0].turns.push(t.id);
        }

        for (idx, b) in data.buildings.iter().enumerate() {
            m.buildings
                .push(make::make_building(b, BuildingID(idx), &bounds, &m.roads));
        }

        for (idx, p) in data.parcels.iter().enumerate() {
            m.parcels.push(Parcel {
                id: ParcelID(idx),
                points: p.points
                    .iter()
                    .map(|coord| geometry::gps_to_screen_space(coord, &bounds))
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
