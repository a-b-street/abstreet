use ezgui::{Color, GfxCtx};
use geom::{Circle, PolyLine, Polygon, Pt2D};
use std::collections::BTreeMap;

pub const ROAD_WIDTH: f64 = 5.0;
const INTERSECTION_RADIUS: f64 = 10.0;
const BUILDING_LENGTH: f64 = 30.0;

pub type BuildingID = usize;
pub type IntersectionID = usize;
pub type RoadID = (IntersectionID, IntersectionID);

pub struct Model {
    intersections: BTreeMap<IntersectionID, Intersection>,
    roads: BTreeMap<RoadID, Road>,
    buildings: BTreeMap<BuildingID, Building>,
}

pub struct Intersection {
    center: Pt2D,
}

impl Intersection {
    fn circle(&self) -> Circle {
        Circle::new(self.center, INTERSECTION_RADIUS)
    }
}

pub struct Road {
    i1: IntersectionID,
    i2: IntersectionID,
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
            g.draw_circle(Color::RED, &i.circle());
        }

        for b in self.buildings.values() {
            g.draw_polygon(Color::BLUE, &b.polygon());
        }
    }
}

impl Model {
    pub fn create_i(&mut self, center: Pt2D) {
        let id = self.intersections.len();
        self.intersections.insert(id, Intersection { center });
    }

    pub fn move_i(&mut self, id: IntersectionID, center: Pt2D) {
        self.intersections.get_mut(&id).unwrap().center = center;
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
        self.roads.insert(id, Road { i1, i2 });
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
