use abstutil::{deserialize_btreemap, serialize_btreemap, write_json};
use dimensioned::si;
use ezgui::{Color, GfxCtx};
use geom::{Circle, LonLat, PolyLine, Polygon, Pt2D};
use map_model::{raw_data, LaneType, RoadSpec};
use std::collections::BTreeMap;

pub const ROAD_WIDTH: f64 = 5.0;
const INTERSECTION_RADIUS: f64 = 10.0;
const BUILDING_LENGTH: f64 = 30.0;

pub type BuildingID = usize;
pub type IntersectionID = usize;
pub type RoadID = (IntersectionID, IntersectionID);

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
    has_traffic_signal: bool,
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
}

impl Road {
    fn polygon(&self, model: &Model) -> Polygon {
        PolyLine::new(vec![
            model.intersections[&self.i1].center,
            model.intersections[&self.i2].center,
        ]).make_polygons(ROAD_WIDTH)
        .unwrap()
    }
}

#[derive(Serialize, Deserialize)]
pub struct Building {
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

    pub fn draw(&self, g: &mut GfxCtx) {
        g.clear(Color::WHITE);

        for r in self.roads.values() {
            g.draw_polygon(Color::BLACK, &r.polygon(self));
        }

        for i in self.intersections.values() {
            g.draw_circle(
                if i.has_traffic_signal {
                    Color::GREEN
                } else {
                    Color::RED
                },
                &i.circle(),
            );
        }

        for b in self.buildings.values() {
            g.draw_polygon(Color::BLUE, &b.polygon());
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
            map.roads.push(raw_data::Road {
                points: vec![
                    pt(self.intersections[&r.i1].center),
                    pt(self.intersections[&r.i2].center),
                ],
                osm_tags,
                osm_way_id: idx as i64,
            });
        }

        for i in self.intersections.values() {
            map.intersections.push(raw_data::Intersection {
                point: pt(i.center),
                elevation: 0.0 * si::M,
                has_traffic_signal: i.has_traffic_signal,
            });
        }

        for (idx, b) in self.buildings.values().enumerate() {
            map.buildings.push(raw_data::Building {
                // TODO Duplicate points :(
                points: b.polygon().points().into_iter().map(|p| pt(p)).collect(),
                osm_tags: BTreeMap::new(),
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
                has_traffic_signal: false,
            },
        );
    }

    pub fn move_i(&mut self, id: IntersectionID, center: Pt2D) {
        self.intersections.get_mut(&id).unwrap().center = center;
    }

    pub fn toggle_i_type(&mut self, id: IntersectionID) {
        let i = self.intersections.get_mut(&id).unwrap();
        i.has_traffic_signal = !i.has_traffic_signal;
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

    pub fn remove_road(&mut self, id: RoadID) {
        self.roads.remove(&id);
    }

    pub fn mouseover_road(&self, pt: Pt2D) -> Option<RoadID> {
        for (id, r) in &self.roads {
            if r.polygon(self).contains_pt(pt) {
                return Some(*id);
            }
        }
        None
    }
}

impl Model {
    pub fn create_b(&mut self, center: Pt2D) {
        let id = self.buildings.len();
        self.buildings.insert(id, Building { center });
    }

    pub fn move_b(&mut self, id: IntersectionID, center: Pt2D) {
        self.buildings.get_mut(&id).unwrap().center = center;
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
