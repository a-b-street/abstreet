use abstutil::{read_binary, Timer};
use viewer::World;
use ezgui::{EventCtx, Prerender, Color, GfxCtx, Text};
use geom::{Circle, Bounds, Distance, LonLat, PolyLine, Polygon, Pt2D};
use map_model::raw_data::{StableIntersectionID, StableRoadID};
use map_model::{raw_data, IntersectionType, LaneType, RoadSpec, LANE_THICKNESS};
use std::collections::{HashSet, BTreeMap};
use std::mem;

const INTERSECTION_RADIUS: Distance = Distance::const_meters(5.0);
const BUILDING_LENGTH: Distance = Distance::const_meters(30.0);
const CENTER_LINE_THICKNESS: Distance = Distance::const_meters(0.5);

const HIGHLIGHT_COLOR: Color = Color::CYAN;

pub type BuildingID = usize;
pub type Direction = bool;
const FORWARDS: Direction = true;
const BACKWARDS: Direction = false;

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum ID {
    Building(BuildingID),
    Intersection(StableIntersectionID),
    Lane(StableRoadID, Direction, usize),
}

impl viewer::ObjectID for ID {
    fn zorder(&self) -> usize {
        match self {
            ID::Lane(_, _, _) => 0,
            ID::Intersection(_) => 1,
            ID::Building(_) => 2,
        }
    }
}

pub struct Model {
    pub name: Option<String>,
    intersections: BTreeMap<StableIntersectionID, Intersection>,
    roads: BTreeMap<StableRoadID, Road>,
    buildings: BTreeMap<BuildingID, Building>,

    world: World<ID>,
}

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

struct Road {
    i1: StableIntersectionID,
    i2: StableIntersectionID,
    lanes: RoadSpec,
    fwd_label: Option<String>,
    back_label: Option<String>,
}

impl Road {
    fn polygons(&self, model: &Model) -> Vec<(Direction, usize, Polygon, Color)> {
        let base = PolyLine::new(vec![
            model.intersections[&self.i1].center,
            model.intersections[&self.i2].center,
        ]);

        let mut result = Vec::new();

        for (idx, lt) in self.lanes.fwd.iter().enumerate() {
            let polygon = base
                .shift_right(LANE_THICKNESS * ((idx as f64) + 0.5))
                .unwrap()
                .make_polygons(LANE_THICKNESS);
            result.push((FORWARDS, idx, polygon, Road::lt_to_color(*lt)));
        }
        for (idx, lt) in self.lanes.back.iter().enumerate() {
            let polygon = base
                .shift_left(LANE_THICKNESS * ((idx as f64) + 0.5))
                .unwrap()
                .make_polygons(LANE_THICKNESS);
            result.push((BACKWARDS, idx, polygon, Road::lt_to_color(*lt)));
        }

        result
    }

    /*fn draw(&self, model: &Model, g: &mut GfxCtx, highlight_fwd: bool, highlight_back: bool) {
        g.draw_polygon(Color::YELLOW, &base.make_polygons(CENTER_LINE_THICKNESS));

        if let Some(ref label) = self.fwd_label {
            g.draw_text_at(
                &Text::from(Line(label)),
                self.polygon(FORWARDS, model).center(),
            );
        }
        if let Some(ref label) = self.back_label {
            g.draw_text_at(
                &Text::from(Line(label)),
                self.polygon(BACKWARDS, model).center(),
            );
        }
    }*/

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
            world: World::new(&Bounds::new()),
        }
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        g.clear(Color::WHITE);

        self.world.draw(g, &HashSet::new());

        // TODO HIGHLIGHT_COLOR

        // TODO Always draw labels?
        /*if let Some(ref label) = i.label {
            g.draw_text_at(&Text::from(Line(label)), i.center);
        }*/
    }

    pub fn mouseover_something(&self, ctx: &EventCtx) -> Option<ID> {
        self.world.mouseover_something(ctx, &HashSet::new())
    }

    fn compute_bounds(&self) -> Bounds {
        let mut bounds = Bounds::new();
        for b in self.buildings.values() {
            for pt in b.polygon().points() {
                bounds.update(*pt);
            }
        }
        for i in self.intersections.values() {
            bounds.update(i.center);
        }
        bounds
    }

    // Returns path to raw map
    pub fn export(&self) -> String {
        let mut map = raw_data::Map::blank();

        fn gps(p: Pt2D) -> LonLat {
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
                    center_points: vec![
                        self.intersections[&r.i1].center,
                        self.intersections[&r.i2].center,
                    ],
                    orig_id: raw_data::OriginalRoad {
                        pt1: gps(self.intersections[&r.i1].center),
                        pt2: gps(self.intersections[&r.i2].center),
                    },
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
                    point: i.center,
                    orig_id: raw_data::OriginalIntersection {
                        point: gps(i.center),
                    },
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
                polygon: b.polygon(),
                osm_tags,
                osm_way_id: idx as i64,
                parking: None,
            });
        }

        // Leave gps_bounds alone. We'll get nonsense answers when converting back to it, which is
        // fine.
        map.boundary_polygon = self.compute_bounds().get_rectangle();

        let path = abstutil::path_raw_map(self.name.as_ref().expect("Model hasn't been named yet"));
        abstutil::write_binary(&path, &map).expect(&format!("Saving {} failed", path));
        println!("Exported {}", path);
        path
    }

    // TODO Directly use raw_data and get rid of Model? Might be more maintainable long-term.
    pub fn import(path: &str, exclude_bldgs: bool, prerender: &Prerender) -> Model {
        let data: raw_data::Map = read_binary(path, &mut Timer::new("load map")).unwrap();

        let mut m = Model::new();
        m.name = Some(abstutil::basename(path));

        for (id, raw_i) in &data.intersections {
            let i = Intersection {
                center: raw_i.point,
                intersection_type: raw_i.intersection_type,
                label: raw_i.label.clone(),
            };
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
        }

        if !exclude_bldgs {
            for (idx, b) in data.buildings.iter().enumerate() {
                let b = Building {
                    label: None,
                    center: b.polygon.center(),
                };
                m.buildings.insert(idx, b);
            }
        }

        m.recompute_world(prerender);

        m
    }

    fn recompute_world(&mut self, prerender: &Prerender) {
        self.world = World::new(&self.compute_bounds());

        for (id, b) in &self.buildings {
            self.world.add_obj(
                prerender,
                ID::Building(*id),
                b.polygon(),
                Color::BLUE,
                // TODO Always show its label?
                Text::new());
        }

        for (id, i) in &self.intersections {
            self.world.add_obj(
                prerender,
                ID::Intersection(*id),
                i.circle().to_polygon(),
                match i.intersection_type {
                    IntersectionType::TrafficSignal => Color::GREEN,
                    IntersectionType::StopSign => Color::RED,
                    IntersectionType::Border => Color::BLUE,
                },
                Text::new());
        }

        for (id, r) in &self.roads {
            for (dir, idx, poly, color) in r.polygons(self) {
                self.world.add_obj(
                    prerender,
                    ID::Lane(*id, dir, idx),
                    poly,
                    color,
                    Text::new());
            }
        }
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
