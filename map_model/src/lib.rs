// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

extern crate ordered_float;
extern crate protobuf;
extern crate serde;
#[macro_use]
extern crate serde_derive;

use ordered_float::NotNaN;
use protobuf::error::ProtobufError;
use protobuf::{CodedInputStream, CodedOutputStream, Message};
use std::collections::HashMap;
use std::f64;
use std::fs::File;

pub mod pb;

pub fn write_pb(map: &pb::Map, path: &str) -> Result<(), ProtobufError> {
    let mut file = File::create(path)?;
    let mut cos = CodedOutputStream::new(&mut file);
    map.write_to(&mut cos)?;
    cos.flush()?;
    Ok(())
}

pub fn load_pb(path: &str) -> Result<pb::Map, ProtobufError> {
    let mut file = File::open(path)?;
    let mut cis = CodedInputStream::new(&mut file);
    let mut map = pb::Map::new();
    map.merge_from(&mut cis)?;
    Ok(map)
}

// This isn't opinionated about what the (x, y) represents. Could be GPS coordinates, could be
// screen-space.
// TODO but actually, different types to represent GPS and screen space would be awesome.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Pt2D {
    x_nan: NotNaN<f64>,
    y_nan: NotNaN<f64>,
}

impl Pt2D {
    pub fn new(x: f64, y: f64) -> Pt2D {
        Pt2D {
            x_nan: NotNaN::new(x).unwrap(),
            y_nan: NotNaN::new(y).unwrap(),
        }
    }

    pub fn zero() -> Pt2D {
        Pt2D::new(0.0, 0.0)
    }

    pub fn x(&self) -> f64 {
        self.x_nan.into_inner()
    }

    pub fn y(&self) -> f64 {
        self.y_nan.into_inner()
    }

    // Interprets the Pt2D as GPS coordinates, using Haversine distance
    pub fn gps_dist_meters(&self, other: &Pt2D) -> f64 {
        let earth_radius_m = 6371000.0;
        let lon1 = self.x().to_radians();
        let lon2 = other.x().to_radians();
        let lat1 = self.y().to_radians();
        let lat2 = other.y().to_radians();

        let delta_lat = lat2 - lat1;
        let delta_lon = lon2 - lon1;

        let a = (delta_lat / 2.0).sin().powi(2)
            + (delta_lon / 2.0).sin().powi(2) * lat1.cos() * lat2.cos();
        let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
        earth_radius_m * c
    }
}

impl<'a> From<&'a pb::Coordinate> for Pt2D {
    fn from(pt: &pb::Coordinate) -> Self {
        Pt2D::new(pt.get_longitude(), pt.get_latitude())
    }
}

impl From<[f64; 2]> for Pt2D {
    fn from(pt: [f64; 2]) -> Self {
        Pt2D::new(pt[0], pt[1])
    }
}

// TODO argh, use this in kml too
#[derive(Debug)]
pub struct Bounds {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

impl Bounds {
    pub fn new() -> Bounds {
        Bounds {
            min_x: f64::MAX,
            min_y: f64::MAX,
            max_x: f64::MIN,
            max_y: f64::MIN,
        }
    }

    pub fn update(&mut self, x: f64, y: f64) {
        self.min_x = self.min_x.min(x);
        self.max_x = self.max_x.max(x);
        self.min_y = self.min_y.min(y);
        self.max_y = self.max_y.max(y);
    }

    pub fn update_pt(&mut self, pt: &Pt2D) {
        self.update(pt.x(), pt.y());
    }

    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.min_x && x <= self.max_x && y >= self.min_y && y <= self.max_y
    }
}

// TODO reconsider pub usize. maybe outside world shouldnt know.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct RoadID(pub usize);
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct IntersectionID(pub usize);
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct TurnID(pub usize);
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct BuildingID(pub usize);
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct ParcelID(pub usize);

pub struct Map {
    roads: Vec<Road>,
    intersections: Vec<Intersection>,
    turns: Vec<Turn>,
    buildings: Vec<Building>,
    parcels: Vec<Parcel>,

    pt_to_intersection: HashMap<Pt2D, IntersectionID>,
    intersection_to_roads: HashMap<IntersectionID, Vec<RoadID>>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum LaneType {
    Driving,
    Parking,
    Sidewalk,
}

#[derive(Debug)]
pub struct Road {
    pub id: RoadID,
    pub osm_tags: Vec<String>,
    pub osm_way_id: i64,
    lane_type: LaneType,

    // Ideally all of these would just become translated center points immediately, but this is
    // hard due to the polyline problem.

    // All roads are two-way (since even one-way streets have sidewalks on both sides). Offset 0 is
    // the centermost lane on each side, then it counts up.
    offset: u8,
    // The orientation is implied by the order of these points
    pub points: Vec<Pt2D>,
    // Should this lane own the drawing of the yellow center lines? For two-way roads, this is
    // arbitrarily grouped with one of the lanes. Ideally it would be owned by something else.
    pub use_yellow_center_lines: bool,
    // Ugly hack, preserving whether the original road geometry represents a one-way road or not.
    pub one_way_road: bool,
    // Need to remember this just for detecting U-turns here.
    other_side: Option<RoadID>,
}

impl PartialEq for Road {
    fn eq(&self, other: &Road) -> bool {
        self.id == other.id
    }
}

#[derive(Debug)]
pub struct Intersection {
    pub id: IntersectionID,
    pub point: Pt2D,
    pub turns: Vec<TurnID>,
    pub elevation_meters: f64,
    pub has_traffic_signal: bool,
}

impl PartialEq for Intersection {
    fn eq(&self, other: &Intersection) -> bool {
        self.id == other.id
    }
}

#[derive(Debug)]
pub struct Turn {
    pub id: TurnID,
    pub parent: IntersectionID,
    pub src: RoadID,
    pub dst: RoadID,
}

impl PartialEq for Turn {
    fn eq(&self, other: &Turn) -> bool {
        self.id == other.id
    }
}

#[derive(Debug)]
pub struct Building {
    pub id: BuildingID,
    pub points: Vec<Pt2D>,
    pub osm_tags: Vec<String>,
    pub osm_way_id: i64,
}

impl PartialEq for Building {
    fn eq(&self, other: &Building) -> bool {
        self.id == other.id
    }
}

#[derive(Debug)]
pub struct Parcel {
    pub id: ParcelID,
    pub points: Vec<Pt2D>,
}

impl PartialEq for Parcel {
    fn eq(&self, other: &Parcel) -> bool {
        self.id == other.id
    }
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
                    if oneway { None } else { Some(1) },
                ),
                //(LaneType::Parking, 1, orig_direction, None),
                //(LaneType::Sidewalk, 2, orig_direction, None),
            ];
            if oneway {
                //lanes.push((LaneType::Sidewalk, 0, reverse_direction, None));
            } else {
                lanes.extend(vec![
                    (LaneType::Driving, 0, reverse_direction, Some(-1)),
                    //(LaneType::Parking, 1, reverse_direction, None),
                    //(LaneType::Sidewalk, 2, reverse_direction, None),
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
                        false
                    },
                    one_way_road: oneway,
                });
            }
        }

        for i in &mut m.intersections {
            // TODO: Figure out why this happens in the huge map
            if m.intersection_to_roads.get(&i.id).is_none() {
                println!("WARNING: intersection {:?} has no roads", i);
                continue;
            }
            let incident_roads = &m.intersection_to_roads[&i.id];
            for src in incident_roads {
                let src_r = &m.roads[src.0];
                if i.point != *src_r.points.last().unwrap() {
                    continue;
                }
                for dst in incident_roads {
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
                    i.turns.push(id);
                }
            }
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

    pub fn get_gps_bounds(&self) -> Bounds {
        let mut b = Bounds::new();
        for r in &self.roads {
            for pt in &r.points {
                b.update_pt(pt);
            }
        }
        for i in &self.intersections {
            b.update_pt(&i.point);
        }
        for bldg in &self.buildings {
            for pt in &bldg.points {
                b.update_pt(pt);
            }
        }
        for p in &self.parcels {
            for pt in &p.points {
                b.update_pt(pt);
            }
        }
        b
    }
}
