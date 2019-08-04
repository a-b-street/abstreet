use aabb_quadtree::QuadTree;
use abstutil::{deserialize_btreemap, read_binary, serialize_btreemap, write_json, Timer};
use ezgui::{Canvas, Color, GfxCtx, Text};
use geom::{Circle, Distance, LonLat, PolyLine, Polygon, Pt2D};
use map_model::raw_data::{StableIntersectionID, StableRoadID};
use map_model::{raw_data, IntersectionType, LaneType, RoadSpec, LANE_THICKNESS};
use serde_derive::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::mem;

const INTERSECTION_RADIUS: Distance = Distance::const_meters(5.0);
const BUILDING_LENGTH: Distance = Distance::const_meters(30.0);
const CENTER_LINE_THICKNESS: Distance = Distance::const_meters(0.5);

const HIGHLIGHT_COLOR: Color = Color::CYAN;

pub type BuildingID = usize;
pub type Direction = bool;

#[derive(Debug, PartialEq)]
pub enum ID {
    Building(BuildingID),
    Intersection(StableIntersectionID),
    Road(StableRoadID),
}

const FORWARDS: Direction = true;
const BACKWARDS: Direction = false;

#[derive(Serialize, Deserialize)]
pub struct Model {
    pub name: Option<String>,
    #[serde(
        serialize_with = "serialize_btreemap",
        deserialize_with = "deserialize_btreemap"
    )]
    intersections: BTreeMap<StableIntersectionID, Intersection>,
    #[serde(
        serialize_with = "serialize_btreemap",
        deserialize_with = "deserialize_btreemap"
    )]
    roads: BTreeMap<StableRoadID, Road>,
    #[serde(
        serialize_with = "serialize_btreemap",
        deserialize_with = "deserialize_btreemap"
    )]
    buildings: BTreeMap<BuildingID, Building>,
}

#[derive(Serialize, Deserialize)]
struct Intersection {
    center: Pt2D,
    intersection_type: IntersectionType,
    label: Option<String>,
}

impl Intersection {
    fn circle(&self) -> Circle {
        Circle::new(self.center, INTERSECTION_RADIUS)
    }
}

#[derive(Serialize, Deserialize)]
struct Road {
    i1: StableIntersectionID,
    i2: StableIntersectionID,
    lanes: RoadSpec,
    fwd_label: Option<String>,
    back_label: Option<String>,
}

impl Road {
    fn polygon(&self, direction: Direction, model: &Model) -> Polygon {
        let pl = PolyLine::new(vec![
            model.intersections[&self.i1].center,
            model.intersections[&self.i2].center,
        ]);
        if direction {
            let width = LANE_THICKNESS * (self.lanes.fwd.len() as f64);
            pl.shift_right(width / 2.0).unwrap().make_polygons(width)
        } else {
            let width = LANE_THICKNESS * (self.lanes.back.len() as f64);
            pl.shift_left(width / 2.0).unwrap().make_polygons(width)
        }
    }

    fn draw(&self, model: &Model, g: &mut GfxCtx, highlight_fwd: bool, highlight_back: bool) {
        let base = PolyLine::new(vec![
            model.intersections[&self.i1].center,
            model.intersections[&self.i2].center,
        ]);

        for (idx, lt) in self.lanes.fwd.iter().enumerate() {
            let polygon = base
                .shift_right(LANE_THICKNESS * ((idx as f64) + 0.5))
                .unwrap()
                .make_polygons(LANE_THICKNESS);
            g.draw_polygon(
                if highlight_fwd {
                    HIGHLIGHT_COLOR
                } else {
                    Road::lt_to_color(*lt)
                },
                &polygon,
            );
        }
        for (idx, lt) in self.lanes.back.iter().enumerate() {
            let polygon = base
                .shift_left(LANE_THICKNESS * ((idx as f64) + 0.5))
                .unwrap()
                .make_polygons(LANE_THICKNESS);
            g.draw_polygon(
                if highlight_back {
                    HIGHLIGHT_COLOR
                } else {
                    Road::lt_to_color(*lt)
                },
                &polygon,
            );
        }

        g.draw_polygon(Color::YELLOW, &base.make_polygons(CENTER_LINE_THICKNESS));

        if let Some(ref label) = self.fwd_label {
            g.draw_text_at(
                &Text::from_line(label.to_string()),
                self.polygon(FORWARDS, model).center(),
            );
        }
        if let Some(ref label) = self.back_label {
            g.draw_text_at(
                &Text::from_line(label.to_string()),
                self.polygon(BACKWARDS, model).center(),
            );
        }
    }

    // Copied from render/lane.rs. :(
    fn lt_to_color(lt: LaneType) -> Color {
        match lt {
            LaneType::Driving => Color::BLACK,
            LaneType::Bus => Color::rgb(190, 74, 76),
            LaneType::Parking => Color::grey(0.2),
            LaneType::Sidewalk => Color::grey(0.8),
            LaneType::Biking => Color::rgb(15, 125, 75),
        }
    }
}

#[derive(Serialize, Deserialize)]
struct Building {
    label: Option<String>,
    center: Pt2D,
}

impl Building {
    fn polygon(&self) -> Polygon {
        Polygon::rectangle(self.center, BUILDING_LENGTH, BUILDING_LENGTH)
    }
}

impl Model {
    pub fn new() -> Model {
        Model {
            name: None,
            intersections: BTreeMap::new(),
            roads: BTreeMap::new(),
            buildings: BTreeMap::new(),
        }
    }

    pub fn draw(&self, g: &mut GfxCtx, quadtree: Option<&QuadTree<ID>>) {
        g.clear(Color::WHITE);

        let mut roads: Vec<StableRoadID> = Vec::new();
        let mut buildings: Vec<BuildingID> = Vec::new();
        let mut intersections: Vec<StableIntersectionID> = Vec::new();
        if let Some(ref qt) = quadtree {
            let bbox = g.get_screen_bounds().as_bbox();
            for &(id, _, _) in &qt.query(bbox) {
                match *id {
                    ID::Road(id) => {
                        roads.push(id);
                    }
                    ID::Building(id) => {
                        buildings.push(id);
                    }
                    ID::Intersection(id) => {
                        intersections.push(id);
                    }
                }
            }
        } else {
            roads.extend(self.roads.keys());
            buildings.extend(self.buildings.keys());
            intersections.extend(self.intersections.keys());
        }

        let selected = self.mouseover_something(g.canvas, quadtree);
        let current_r = match selected {
            Some(ID::Road(r)) => self.mouseover_road(r, g.get_cursor_in_map_space().unwrap()),
            _ => None,
        };

        for id in roads {
            let r = &self.roads[&id];
            r.draw(
                self,
                g,
                Some((id, FORWARDS)) == current_r,
                Some((id, BACKWARDS)) == current_r,
            );
        }

        for id in intersections {
            let i = &self.intersections[&id];
            let color = if Some(ID::Intersection(id)) == selected {
                HIGHLIGHT_COLOR
            } else {
                match i.intersection_type {
                    IntersectionType::TrafficSignal => Color::GREEN,
                    IntersectionType::StopSign => Color::RED,
                    IntersectionType::Border => Color::BLUE,
                }
            };
            g.draw_circle(color, &i.circle());

            if let Some(ref label) = i.label {
                g.draw_text_at(&Text::from_line(label.to_string()), i.center);
            }
        }

        for id in buildings {
            let b = &self.buildings[&id];
            let color = if Some(ID::Building(id)) == selected {
                HIGHLIGHT_COLOR
            } else {
                Color::BLUE
            };
            g.draw_polygon(color, &b.polygon());

            if let Some(ref label) = b.label {
                g.draw_text_at(&Text::from_line(label.to_string()), b.center);
            }
        }
    }

    pub fn mouseover_something(
        &self,
        canvas: &Canvas,
        quadtree: Option<&QuadTree<ID>>,
    ) -> Option<ID> {
        let cursor = canvas.get_cursor_in_map_space()?;

        // TODO Duplicated with draw
        let mut roads: Vec<StableRoadID> = Vec::new();
        let mut buildings: Vec<BuildingID> = Vec::new();
        let mut intersections: Vec<StableIntersectionID> = Vec::new();
        if let Some(ref qt) = quadtree {
            let bbox = canvas.get_screen_bounds().as_bbox();
            for &(id, _, _) in &qt.query(bbox) {
                match *id {
                    ID::Road(id) => {
                        roads.push(id);
                    }
                    ID::Building(id) => {
                        buildings.push(id);
                    }
                    ID::Intersection(id) => {
                        intersections.push(id);
                    }
                }
            }
        } else {
            roads.extend(self.roads.keys());
            buildings.extend(self.buildings.keys());
            intersections.extend(self.intersections.keys());
        }

        for id in intersections {
            let i = &self.intersections[&id];
            if i.circle().contains_pt(cursor) {
                return Some(ID::Intersection(id));
            }
        }

        for id in buildings {
            let b = &self.buildings[&id];
            if b.polygon().contains_pt(cursor) {
                return Some(ID::Building(id));
            }
        }

        for id in roads {
            if self.mouseover_road(id, cursor).is_some() {
                return Some(ID::Road(id));
            }
        }

        None
    }

    pub fn save(&self) {
        let path =
            abstutil::path_synthetic_map(self.name.as_ref().expect("Model hasn't been named yet"));
        write_json(&path, self).expect(&format!("Saving {} failed", path));
        println!("Saved {}", path);
    }

    // Returns path to raw map
    pub fn export(&self) -> String {
        let mut map = raw_data::Map::blank();
        map.coordinates_in_world_space = true;

        fn pt(p: Pt2D) -> LonLat {
            LonLat::new(p.x(), p.y())
        }

        for (id, r) in &self.roads {
            let mut osm_tags = BTreeMap::new();
            osm_tags.insert("synthetic_lanes".to_string(), r.lanes.to_string());
            if let Some(ref label) = r.fwd_label {
                osm_tags.insert("fwd_label".to_string(), label.to_string());
            }
            if let Some(ref label) = r.back_label {
                osm_tags.insert("back_label".to_string(), label.to_string());
            }
            map.roads.insert(
                *id,
                raw_data::Road {
                    i1: r.i1,
                    i2: r.i2,
                    points: vec![
                        pt(self.intersections[&r.i1].center),
                        pt(self.intersections[&r.i2].center),
                    ],
                    osm_tags,
                    osm_way_id: id.0 as i64,
                    parking_lane_fwd: r.lanes.fwd.contains(&LaneType::Parking),
                    parking_lane_back: r.lanes.back.contains(&LaneType::Parking),
                },
            );
        }

        for (id, i) in &self.intersections {
            map.intersections.insert(
                *id,
                raw_data::Intersection {
                    point: pt(i.center),
                    intersection_type: i.intersection_type,
                    label: i.label.clone(),
                },
            );
        }

        for (idx, b) in self.buildings.values().enumerate() {
            let mut osm_tags = BTreeMap::new();
            if let Some(ref label) = b.label {
                osm_tags.insert("label".to_string(), label.to_string());
            }
            map.buildings.push(raw_data::Building {
                // TODO Duplicate points :(
                points: b.polygon().points().iter().map(|p| pt(*p)).collect(),
                osm_tags,
                osm_way_id: idx as i64,
            });
        }

        map.compute_gps_bounds();
        map.boundary_polygon = map.gps_bounds.get_corners();
        // Close off the polygon
        map.boundary_polygon.push(map.boundary_polygon[0]);

        let path = abstutil::path_raw_map(self.name.as_ref().expect("Model hasn't been named yet"));
        abstutil::write_binary(&path, &map).expect(&format!("Saving {} failed", path));
        println!("Exported {}", path);
        path
    }

    // TODO Directly use raw_data and get rid of Model? Might be more maintainable long-term.
    pub fn import(path: &str) -> (Model, QuadTree<ID>) {
        let data: raw_data::Map = read_binary(path, &mut Timer::new("load map")).unwrap();

        let mut m = Model::new();
        let mut quadtree = QuadTree::default(data.gps_bounds.to_bounds().as_bbox());

        for (id, raw_i) in &data.intersections {
            let center = Pt2D::from_gps(raw_i.point, &data.gps_bounds).unwrap();
            let i = Intersection {
                center,
                intersection_type: raw_i.intersection_type,
                label: raw_i.label.clone(),
            };
            quadtree.insert_with_box(ID::Intersection(*id), i.circle().get_bounds().as_bbox());
            m.intersections.insert(*id, i);
        }

        for (id, r) in &data.roads {
            let (i1, i2) = (r.i1, r.i2);
            m.roads.insert(
                *id,
                Road {
                    i1,
                    i2,
                    lanes: r.get_spec(),
                    fwd_label: r.osm_tags.get("fwd_label").cloned(),
                    back_label: r.osm_tags.get("back_label").cloned(),
                },
            );
            let pl = PolyLine::new(vec![
                m.intersections[&i1].center,
                m.intersections[&i2].center,
            ]);
            quadtree.insert_with_box(
                ID::Road(*id),
                pl.make_polygons(LANE_THICKNESS * 6.0)
                    .get_bounds()
                    .as_bbox(),
            );
        }

        // TODO Too slow!
        /*for (idx, b) in data.buildings.iter().enumerate() {
            let b = Building {
                label: None,
                center: Pt2D::center(&data.gps_bounds.must_convert(&b.points)),
            };
            quadtree.insert_with_box(ID::Building(idx), b.polygon().get_bounds().as_bbox());
            m.buildings.insert(idx, b);
        }*/

        (m, quadtree)
    }
}

impl Model {
    pub fn create_i(&mut self, center: Pt2D) {
        let id = StableIntersectionID(self.intersections.len());
        self.intersections.insert(
            id,
            Intersection {
                center,
                intersection_type: IntersectionType::StopSign,
                label: None,
            },
        );
    }

    pub fn move_i(&mut self, id: StableIntersectionID, center: Pt2D) {
        self.intersections.get_mut(&id).unwrap().center = center;
    }

    pub fn set_i_label(&mut self, id: StableIntersectionID, label: String) {
        self.intersections.get_mut(&id).unwrap().label = Some(label);
    }

    pub fn get_i_label(&self, id: StableIntersectionID) -> Option<String> {
        self.intersections[&id].label.clone()
    }

    pub fn toggle_i_type(&mut self, id: StableIntersectionID) {
        let i = self.intersections.get_mut(&id).unwrap();
        i.intersection_type = match i.intersection_type {
            IntersectionType::StopSign => IntersectionType::TrafficSignal,
            IntersectionType::TrafficSignal => {
                let num_roads = self
                    .roads
                    .values()
                    .filter(|r| r.i1 == id || r.i2 == id)
                    .count();
                if num_roads == 1 {
                    IntersectionType::Border
                } else {
                    IntersectionType::StopSign
                }
            }
            IntersectionType::Border => IntersectionType::StopSign,
        };
    }

    pub fn remove_i(&mut self, id: StableIntersectionID) {
        for r in self.roads.values() {
            if r.i1 == id || r.i2 == id {
                println!("Can't delete intersection used by roads");
                return;
            }
        }
        self.intersections.remove(&id);
    }

    pub fn get_i_center(&self, id: StableIntersectionID) -> Pt2D {
        self.intersections[&id].center
    }
}

impl Model {
    pub fn create_road(&mut self, i1: StableIntersectionID, i2: StableIntersectionID) {
        if self
            .roads
            .values()
            .any(|r| (r.i1 == i1 && r.i2 == i2) || (r.i1 == i2 && r.i2 == i1))
        {
            println!("Road already exists");
            return;
        }
        self.roads.insert(
            StableRoadID(self.roads.len()),
            Road {
                i1,
                i2,
                lanes: RoadSpec {
                    fwd: vec![LaneType::Driving, LaneType::Parking, LaneType::Sidewalk],
                    back: vec![LaneType::Driving, LaneType::Parking, LaneType::Sidewalk],
                },
                fwd_label: None,
                back_label: None,
            },
        );
    }

    pub fn edit_lanes(&mut self, id: StableRoadID, spec: String) {
        if let Some(s) = RoadSpec::parse(spec.clone()) {
            self.roads.get_mut(&id).unwrap().lanes = s;
        } else {
            println!("Bad RoadSpec: {}", spec);
        }
    }

    pub fn swap_lanes(&mut self, id: StableRoadID) {
        let lanes = &mut self.roads.get_mut(&id).unwrap().lanes;
        mem::swap(&mut lanes.fwd, &mut lanes.back);
    }

    pub fn set_r_label(&mut self, pair: (StableRoadID, Direction), label: String) {
        let r = self.roads.get_mut(&pair.0).unwrap();
        if pair.1 {
            r.fwd_label = Some(label);
        } else {
            r.back_label = Some(label);
        }
    }

    pub fn get_r_label(&self, pair: (StableRoadID, Direction)) -> Option<String> {
        let r = &self.roads[&pair.0];
        if pair.1 {
            r.fwd_label.clone()
        } else {
            r.back_label.clone()
        }
    }

    pub fn remove_road(&mut self, id: StableRoadID) {
        self.roads.remove(&id);
    }

    // TODO Make (StableRoadID, Direction) be the primitive, I guess.
    pub fn mouseover_road(&self, id: StableRoadID, pt: Pt2D) -> Option<(StableRoadID, Direction)> {
        let r = &self.roads[&id];
        if r.polygon(FORWARDS, self).contains_pt(pt) {
            return Some((id, FORWARDS));
        }
        if r.polygon(BACKWARDS, self).contains_pt(pt) {
            return Some((id, BACKWARDS));
        }
        None
    }

    pub fn get_lanes(&self, id: StableRoadID) -> String {
        self.roads[&id].lanes.to_string()
    }
}

impl Model {
    pub fn create_b(&mut self, center: Pt2D) {
        let id = self.buildings.len();
        self.buildings.insert(
            id,
            Building {
                center,
                label: None,
            },
        );
    }

    pub fn move_b(&mut self, id: BuildingID, center: Pt2D) {
        self.buildings.get_mut(&id).unwrap().center = center;
    }

    pub fn set_b_label(&mut self, id: BuildingID, label: String) {
        self.buildings.get_mut(&id).unwrap().label = Some(label);
    }

    pub fn get_b_label(&self, id: BuildingID) -> Option<String> {
        self.buildings[&id].label.clone()
    }

    pub fn remove_b(&mut self, id: BuildingID) {
        self.buildings.remove(&id);
    }
}
