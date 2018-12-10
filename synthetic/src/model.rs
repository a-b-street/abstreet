use abstutil::{deserialize_btreemap, serialize_btreemap, write_json};
use dimensioned::si;
use ezgui::{Canvas, Color, GfxCtx, Text};
use geom::{Circle, LonLat, PolyLine, Polygon, Pt2D};
use map_model::{raw_data, IntersectionType, LaneType, RoadSpec, LANE_THICKNESS};
use serde_derive::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::mem;

const INTERSECTION_RADIUS: f64 = 10.0;
const BUILDING_LENGTH: f64 = 30.0;
const CENTER_LINE_THICKNESS: f64 = 0.5;

const HIGHLIGHT_COLOR: Color = Color::CYAN;

pub type BuildingID = usize;
pub type IntersectionID = usize;
pub type RoadID = (IntersectionID, IntersectionID);
pub type Direction = bool;

const FORWARDS: Direction = true;
const BACKWARDS: Direction = false;

#[derive(Serialize, Deserialize)]
pub struct Model {
    pub name: Option<String>,
    #[serde(
        serialize_with = "serialize_btreemap",
        deserialize_with = "deserialize_btreemap"
    )]
    intersections: BTreeMap<IntersectionID, Intersection>,
    #[serde(
        serialize_with = "serialize_btreemap",
        deserialize_with = "deserialize_btreemap"
    )]
    roads: BTreeMap<RoadID, Road>,
    #[serde(
        serialize_with = "serialize_btreemap",
        deserialize_with = "deserialize_btreemap"
    )]
    buildings: BTreeMap<BuildingID, Building>,
}

#[derive(Serialize, Deserialize)]
pub struct Intersection {
    center: Pt2D,
    intersection_type: IntersectionType,
}

impl Intersection {
    fn circle(&self) -> Circle {
        Circle::new(self.center, INTERSECTION_RADIUS)
    }
}

#[derive(Serialize, Deserialize)]
pub struct Road {
    i1: IntersectionID,
    i2: IntersectionID,
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
            pl.shift_blindly(width / 2.0).make_polygons_blindly(width)
        } else {
            let width = LANE_THICKNESS * (self.lanes.back.len() as f64);
            pl.reversed()
                .shift_blindly(width / 2.0)
                .make_polygons_blindly(width)
        }
    }

    fn draw(
        &self,
        model: &Model,
        g: &mut GfxCtx,
        canvas: &Canvas,
        highlight_fwd: bool,
        highlight_back: bool,
    ) {
        let base = PolyLine::new(vec![
            model.intersections[&self.i1].center,
            model.intersections[&self.i2].center,
        ]);

        for (idx, lt) in self.lanes.fwd.iter().enumerate() {
            let polygon = base
                .shift_blindly(((idx as f64) + 0.5) * LANE_THICKNESS)
                .make_polygons_blindly(LANE_THICKNESS);
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
                .reversed()
                .shift_blindly(((idx as f64) + 0.5) * LANE_THICKNESS)
                .make_polygons_blindly(LANE_THICKNESS);
            g.draw_polygon(
                if highlight_back {
                    HIGHLIGHT_COLOR
                } else {
                    Road::lt_to_color(*lt)
                },
                &polygon,
            );
        }

        g.draw_polygon(
            Color::YELLOW,
            &base.make_polygons_blindly(CENTER_LINE_THICKNESS),
        );

        if let Some(ref label) = self.fwd_label {
            let mut txt = Text::new();
            txt.add_line(label.to_string());
            canvas.draw_text_at(g, txt, self.polygon(FORWARDS, model).center());
        }
        if let Some(ref label) = self.back_label {
            let mut txt = Text::new();
            txt.add_line(label.to_string());
            canvas.draw_text_at(g, txt, self.polygon(BACKWARDS, model).center());
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
pub struct Building {
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

    pub fn draw(&self, g: &mut GfxCtx, canvas: &Canvas) {
        g.clear(Color::WHITE);

        let cursor = canvas.get_cursor_in_map_space();
        let current_i = self.mouseover_intersection(cursor);
        let current_b = self.mouseover_building(cursor);
        let current_r = self.mouseover_road(cursor);

        for (id, r) in &self.roads {
            r.draw(
                self,
                g,
                canvas,
                Some((*id, FORWARDS)) == current_r,
                Some((*id, BACKWARDS)) == current_r,
            );
        }

        for (id, i) in &self.intersections {
            let color = if Some(*id) == current_i {
                HIGHLIGHT_COLOR
            } else {
                match i.intersection_type {
                    IntersectionType::TrafficSignal => Color::GREEN,
                    IntersectionType::StopSign => Color::RED,
                    IntersectionType::Border => Color::BLUE,
                }
            };
            g.draw_circle(color, &i.circle());
        }

        for (id, b) in &self.buildings {
            let color = if Some(*id) == current_b {
                HIGHLIGHT_COLOR
            } else {
                Color::BLUE
            };
            g.draw_polygon(color, &b.polygon());

            if let Some(ref label) = b.label {
                let mut txt = Text::new();
                txt.add_line(label.to_string());
                canvas.draw_text_at(g, txt, b.center);
            }
        }
    }

    pub fn save(&self) {
        let path = format!(
            "../data/synthetic_maps/{}.json",
            self.name.as_ref().expect("Model hasn't been named yet")
        );
        write_json(&path, self).expect(&format!("Saving {} failed", path));
        println!("Saved {}", path);
    }

    pub fn export(&self) {
        let mut map = raw_data::Map::blank();
        map.coordinates_in_world_space = true;

        fn pt(p: Pt2D) -> LonLat {
            LonLat::new(p.x(), p.y())
        }

        for (idx, r) in self.roads.values().enumerate() {
            let mut osm_tags = BTreeMap::new();
            osm_tags.insert("synthetic_lanes".to_string(), r.lanes.to_string());
            if let Some(ref label) = r.fwd_label {
                osm_tags.insert("fwd_label".to_string(), label.to_string());
            }
            if let Some(ref label) = r.back_label {
                osm_tags.insert("back_label".to_string(), label.to_string());
            }
            map.roads.push(raw_data::Road {
                points: vec![
                    pt(self.intersections[&r.i1].center),
                    pt(self.intersections[&r.i2].center),
                ],
                osm_tags,
                osm_way_id: idx as i64,
                parking_lane_fwd: r.lanes.fwd.contains(&LaneType::Parking),
                parking_lane_back: r.lanes.back.contains(&LaneType::Parking),
            });
        }

        for i in self.intersections.values() {
            map.intersections.push(raw_data::Intersection {
                point: pt(i.center),
                elevation: 0.0 * si::M,
                intersection_type: i.intersection_type,
            });
        }

        for (idx, b) in self.buildings.values().enumerate() {
            let mut osm_tags = BTreeMap::new();
            if let Some(ref label) = b.label {
                osm_tags.insert("label".to_string(), label.to_string());
            }
            map.buildings.push(raw_data::Building {
                // TODO Duplicate points :(
                points: b.polygon().points().into_iter().map(pt).collect(),
                osm_tags,
                osm_way_id: idx as i64,
            });
        }

        let path = format!(
            "../data/raw_maps/{}.abst",
            self.name.as_ref().expect("Model hasn't been named yet")
        );
        abstutil::write_binary(&path, &map).expect(&format!("Saving {} failed", path));
        println!("Exported {}", path);
    }
}

impl Model {
    pub fn create_i(&mut self, center: Pt2D) {
        let id = self.intersections.len();
        self.intersections.insert(
            id,
            Intersection {
                center,
                intersection_type: IntersectionType::StopSign,
            },
        );
    }

    pub fn move_i(&mut self, id: IntersectionID, center: Pt2D) {
        self.intersections.get_mut(&id).unwrap().center = center;
    }

    pub fn toggle_i_type(&mut self, id: IntersectionID) {
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

    pub fn remove_i(&mut self, id: IntersectionID) {
        for (i1, i2) in self.roads.keys() {
            if *i1 == id || *i2 == id {
                println!("Can't delete intersection used by roads");
                return;
            }
        }
        self.intersections.remove(&id);
    }

    pub fn get_i_center(&self, id: IntersectionID) -> Pt2D {
        self.intersections[&id].center
    }

    pub fn mouseover_intersection(&self, pt: Pt2D) -> Option<IntersectionID> {
        for (id, i) in &self.intersections {
            if i.circle().contains_pt(pt) {
                return Some(*id);
            }
        }
        None
    }
}

impl Model {
    pub fn create_road(&mut self, i1: IntersectionID, i2: IntersectionID) {
        let id = if i1 < i2 { (i1, i2) } else { (i2, i1) };
        if self.roads.contains_key(&id) {
            println!("Road already exists");
            return;
        }
        self.roads.insert(
            id,
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

    pub fn edit_lanes(&mut self, id: RoadID, spec: String) {
        if let Some(s) = RoadSpec::parse(spec.clone()) {
            self.roads.get_mut(&id).unwrap().lanes = s;
        } else {
            println!("Bad RoadSpec: {}", spec);
        }
    }

    pub fn swap_lanes(&mut self, id: RoadID) {
        let lanes = &mut self.roads.get_mut(&id).unwrap().lanes;
        mem::swap(&mut lanes.fwd, &mut lanes.back);
    }

    pub fn set_r_label(&mut self, pair: (RoadID, Direction), label: String) {
        let r = self.roads.get_mut(&pair.0).unwrap();
        if pair.1 {
            r.fwd_label = Some(label);
        } else {
            r.back_label = Some(label);
        }
    }

    pub fn get_r_label(&self, pair: (RoadID, Direction)) -> Option<String> {
        let r = &self.roads[&pair.0];
        if pair.1 {
            r.fwd_label.clone()
        } else {
            r.back_label.clone()
        }
    }

    pub fn remove_road(&mut self, id: RoadID) {
        self.roads.remove(&id);
    }

    pub fn mouseover_road(&self, pt: Pt2D) -> Option<(RoadID, Direction)> {
        for (id, r) in &self.roads {
            if r.polygon(FORWARDS, self).contains_pt(pt) {
                return Some((*id, FORWARDS));
            }
            if r.polygon(BACKWARDS, self).contains_pt(pt) {
                return Some((*id, BACKWARDS));
            }
        }
        None
    }

    pub fn get_lanes(&self, id: RoadID) -> String {
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

    pub fn mouseover_building(&self, pt: Pt2D) -> Option<BuildingID> {
        for (id, b) in &self.buildings {
            if b.polygon().contains_pt(pt) {
                return Some(*id);
            }
        }
        None
    }
}
