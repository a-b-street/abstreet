use abstutil::{read_binary, Timer};
use ezgui::world::{Object, ObjectID, World};
use ezgui::{Color, EventCtx, GfxCtx, Line, Prerender, Text};
use geom::{Bounds, Circle, Distance, LonLat, PolyLine, Polygon, Pt2D};
use map_model::raw_data::{StableIntersectionID, StableRoadID};
use map_model::{raw_data, IntersectionType, LaneType, RoadSpec, LANE_THICKNESS};
use std::collections::{BTreeMap, HashSet};
use std::mem;

const INTERSECTION_RADIUS: Distance = Distance::const_meters(5.0);
const BUILDING_LENGTH: Distance = Distance::const_meters(30.0);
const CENTER_LINE_THICKNESS: Distance = Distance::const_meters(0.5);

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

impl ObjectID for ID {
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
    // Never reuse IDs, and don't worry about being sequential
    id_counter: usize,

    world: World<ID>,
}

struct Intersection {
    center: Pt2D,
    intersection_type: IntersectionType,
    label: Option<String>,
    roads: HashSet<StableRoadID>,
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
    osm_tags: BTreeMap<String, String>,
}

impl Road {
    fn lanes(&self, id: StableRoadID, model: &Model) -> Vec<Object<ID>> {
        let mut tooltip = Text::new();
        if let Some(name) = self.osm_tags.get("name") {
            tooltip.add(Line(name));
        } else if let Some(name) = self.osm_tags.get("ref") {
            tooltip.add(Line(name));
        } else {
            tooltip.add(Line("some road"));
        }

        let base = PolyLine::new(vec![
            model.intersections[&self.i1].center,
            model.intersections[&self.i2].center,
        ]);
        // Same logic as get_thick_polyline
        let width_right = (self.lanes.fwd.len() as f64) * LANE_THICKNESS;
        let width_left = (self.lanes.back.len() as f64) * LANE_THICKNESS;
        let centered_base = if width_right >= width_left {
            base.shift_right((width_right - width_left) / 2.0).unwrap()
        } else {
            base.shift_left((width_left - width_right) / 2.0).unwrap()
        };

        let mut result = Vec::new();

        for (idx, lt) in self.lanes.fwd.iter().enumerate() {
            let mut obj = Object::new(
                ID::Lane(id, FORWARDS, idx),
                Road::lt_to_color(*lt),
                centered_base
                    .shift_right(LANE_THICKNESS * ((idx as f64) + 0.5))
                    .unwrap()
                    .make_polygons(LANE_THICKNESS),
            );
            if idx == 0 {
                obj = obj.push(
                    Color::YELLOW,
                    centered_base.make_polygons(CENTER_LINE_THICKNESS),
                );
            }
            if idx == self.lanes.fwd.len() / 2 {
                obj = obj.maybe_label(self.fwd_label.clone());
            }
            result.push(obj.tooltip(tooltip.clone()));
        }
        for (idx, lt) in self.lanes.back.iter().enumerate() {
            let mut obj = Object::new(
                ID::Lane(id, BACKWARDS, idx),
                Road::lt_to_color(*lt),
                centered_base
                    .shift_left(LANE_THICKNESS * ((idx as f64) + 0.5))
                    .unwrap()
                    .make_polygons(LANE_THICKNESS),
            );
            if idx == self.lanes.back.len() / 2 {
                obj = obj.maybe_label(self.back_label.clone());
            }
            result.push(obj.tooltip(tooltip.clone()));
        }

        result
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
            id_counter: 0,
            world: World::new(&Bounds::new()),
        }
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        g.clear(Color::WHITE);

        self.world.draw(g);
    }

    pub fn handle_mouseover(&mut self, ctx: &EventCtx) {
        self.world.handle_mouseover(ctx);
    }

    pub fn get_selection(&self) -> Option<ID> {
        self.world.get_selection()
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
                roads: HashSet::new(),
            };
            m.intersections.insert(*id, i);
            m.id_counter = m.id_counter.max(id.0 + 1);
        }

        for (id, r) in &data.roads {
            let (i1, i2) = (r.i1, r.i2);

            if let Some(other) = m
                .roads
                .values()
                .find(|r| (r.i1 == i1 && r.i2 == i2) || (r.i1 == i2 && r.i2 == i1))
            {
                println!("Two roads go between the same intersections...");
                for (k, v1) in &r.osm_tags {
                    let v2 = other
                        .osm_tags
                        .get(k)
                        .cloned()
                        .unwrap_or("MISSING".to_string());
                    println!("  {} = {}   /   {}", k, v1, v2);
                }
                for (k, v2) in &other.osm_tags {
                    if !r.osm_tags.contains_key(k) {
                        println!("  {} = MISSING   /   {}", k, v2);
                    }
                }
                // Strip these out for now
                continue;
            }

            let mut stripped_tags = r.osm_tags.clone();
            stripped_tags.remove("synthetic_lanes");
            stripped_tags.remove("fwd_label");
            stripped_tags.remove("back_label");

            m.roads.insert(
                *id,
                Road {
                    i1,
                    i2,
                    lanes: r.get_spec(),
                    fwd_label: r.osm_tags.get("fwd_label").cloned(),
                    back_label: r.osm_tags.get("back_label").cloned(),
                    osm_tags: stripped_tags,
                },
            );
            m.intersections.get_mut(&i1).unwrap().roads.insert(*id);
            m.intersections.get_mut(&i2).unwrap().roads.insert(*id);
            m.id_counter = m.id_counter.max(id.0 + 1);
        }

        if !exclude_bldgs {
            for (idx, b) in data.buildings.iter().enumerate() {
                let b = Building {
                    label: b.osm_tags.get("label").cloned(),
                    center: b.polygon.center(),
                };
                m.buildings.insert(idx, b);
                m.id_counter = m.id_counter.max(idx + 1);
            }
        }

        m.recompute_world(prerender);

        m
    }

    fn recompute_world(&mut self, prerender: &Prerender) {
        self.world = World::new(&self.compute_bounds());

        for id in self.buildings.keys().cloned().collect::<Vec<_>>() {
            self.bldg_added(id, prerender);
        }
        for id in self.intersections.keys().cloned().collect::<Vec<_>>() {
            self.intersection_added(id, prerender);
        }
        for id in self.roads.keys().cloned().collect::<Vec<_>>() {
            self.road_added(id, prerender);
        }
    }
}

impl Model {
    fn intersection_added(&mut self, id: StableIntersectionID, prerender: &Prerender) {
        let i = &self.intersections[&id];
        self.world.add(
            prerender,
            Object::new(
                ID::Intersection(id),
                match i.intersection_type {
                    IntersectionType::TrafficSignal => Color::GREEN,
                    IntersectionType::StopSign => Color::RED,
                    IntersectionType::Border => Color::BLUE,
                },
                i.circle().to_polygon(),
            )
            .maybe_label(i.label.clone()),
        );
    }

    pub fn create_i(&mut self, center: Pt2D, prerender: &Prerender) {
        let id = StableIntersectionID(self.id_counter);
        self.id_counter += 1;
        self.intersections.insert(
            id,
            Intersection {
                center,
                intersection_type: IntersectionType::StopSign,
                label: None,
                roads: HashSet::new(),
            },
        );

        self.intersection_added(id, prerender);
    }

    pub fn move_i(&mut self, id: StableIntersectionID, center: Pt2D, prerender: &Prerender) {
        self.world.delete(ID::Intersection(id));

        self.intersections.get_mut(&id).unwrap().center = center;

        self.intersection_added(id, prerender);

        // Now update all the roads.
        for r in self.intersections[&id].roads.clone() {
            self.road_deleted(r);
            self.road_added(r, prerender);
        }
    }

    pub fn set_i_label(&mut self, id: StableIntersectionID, label: String, prerender: &Prerender) {
        self.world.delete(ID::Intersection(id));

        self.intersections.get_mut(&id).unwrap().label = Some(label);

        self.intersection_added(id, prerender);
    }

    pub fn get_i_label(&self, id: StableIntersectionID) -> Option<String> {
        self.intersections[&id].label.clone()
    }

    pub fn toggle_i_type(&mut self, id: StableIntersectionID, prerender: &Prerender) {
        self.world.delete(ID::Intersection(id));

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

        self.intersection_added(id, prerender);
    }

    pub fn remove_i(&mut self, id: StableIntersectionID) {
        if !self.intersections[&id].roads.is_empty() {
            println!("Can't delete intersection used by roads");
            return;
        }
        self.intersections.remove(&id);

        self.world.delete(ID::Intersection(id));
    }

    pub fn get_i_center(&self, id: StableIntersectionID) -> Pt2D {
        self.intersections[&id].center
    }
}

impl Model {
    fn road_added(&mut self, id: StableRoadID, prerender: &Prerender) {
        for obj in self.roads[&id].lanes(id, self) {
            self.world.add(prerender, obj);
        }
    }

    fn road_deleted(&mut self, id: StableRoadID) {
        for obj in self.roads[&id].lanes(id, self) {
            self.world.delete(obj.get_id());
        }
    }

    pub fn create_road(
        &mut self,
        i1: StableIntersectionID,
        i2: StableIntersectionID,
        prerender: &Prerender,
    ) {
        if self
            .roads
            .values()
            .any(|r| (r.i1 == i1 && r.i2 == i2) || (r.i1 == i2 && r.i2 == i1))
        {
            println!("Road already exists");
            return;
        }
        let id = StableRoadID(self.id_counter);
        self.id_counter += 1;
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
                osm_tags: BTreeMap::new(),
            },
        );
        self.intersections.get_mut(&i1).unwrap().roads.insert(id);
        self.intersections.get_mut(&i2).unwrap().roads.insert(id);

        self.road_added(id, prerender);
    }

    pub fn edit_lanes(&mut self, id: StableRoadID, spec: String, prerender: &Prerender) {
        self.road_deleted(id);

        if let Some(s) = RoadSpec::parse(spec.clone()) {
            let r = self.roads.get_mut(&id).unwrap();
            r.lanes = s;
        } else {
            println!("Bad RoadSpec: {}", spec);
        }

        self.road_added(id, prerender);
    }

    pub fn swap_lanes(&mut self, id: StableRoadID, prerender: &Prerender) {
        self.road_deleted(id);

        let r = self.roads.get_mut(&id).unwrap();
        let lanes = &mut r.lanes;
        mem::swap(&mut lanes.fwd, &mut lanes.back);
        mem::swap(&mut r.fwd_label, &mut r.back_label);

        self.road_added(id, prerender);
    }

    pub fn set_r_label(
        &mut self,
        pair: (StableRoadID, Direction),
        label: String,
        prerender: &Prerender,
    ) {
        self.road_deleted(pair.0);

        let r = self.roads.get_mut(&pair.0).unwrap();
        if pair.1 {
            r.fwd_label = Some(label);
        } else {
            r.back_label = Some(label);
        }

        self.road_added(pair.0, prerender);
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
        self.road_deleted(id);

        let r = self.roads.remove(&id).unwrap();
        self.intersections.get_mut(&r.i1).unwrap().roads.remove(&id);
        self.intersections.get_mut(&r.i2).unwrap().roads.remove(&id);
    }

    pub fn get_lanes(&self, id: StableRoadID) -> String {
        self.roads[&id].lanes.to_string()
    }

    pub fn get_tags(&self, id: StableRoadID) -> &BTreeMap<String, String> {
        &self.roads[&id].osm_tags
    }
}

impl Model {
    fn bldg_added(&mut self, id: BuildingID, prerender: &Prerender) {
        let b = &self.buildings[&id];
        self.world.add(
            prerender,
            Object::new(ID::Building(id), Color::BLUE, b.polygon()).maybe_label(b.label.clone()),
        );
    }

    pub fn create_b(&mut self, center: Pt2D, prerender: &Prerender) {
        let id = self.id_counter;
        self.id_counter += 1;
        self.buildings.insert(
            id,
            Building {
                center,
                label: None,
            },
        );

        self.bldg_added(id, prerender);
    }

    pub fn move_b(&mut self, id: BuildingID, center: Pt2D, prerender: &Prerender) {
        self.world.delete(ID::Building(id));

        self.buildings.get_mut(&id).unwrap().center = center;

        self.bldg_added(id, prerender);
    }

    pub fn set_b_label(&mut self, id: BuildingID, label: String, prerender: &Prerender) {
        self.world.delete(ID::Building(id));

        let b = self.buildings.get_mut(&id).unwrap();
        b.label = Some(label.clone());

        self.bldg_added(id, prerender);
    }

    pub fn get_b_label(&self, id: BuildingID) -> Option<String> {
        self.buildings[&id].label.clone()
    }

    pub fn remove_b(&mut self, id: BuildingID) {
        self.world.delete(ID::Building(id));

        self.buildings.remove(&id);
    }
}
