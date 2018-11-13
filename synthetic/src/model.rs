use ezgui::{Color, GfxCtx};
use geom::{Circle, Line, Pt2D};
use std::collections::BTreeMap;

pub type IntersectionID = usize;

pub struct Model {
    pub intersections: BTreeMap<IntersectionID, Intersection>,
    pub roads: Vec<Road>,
    buildings: Vec<Building>,
}

pub struct Intersection {
    pub center: Pt2D,
}

impl Intersection {
    fn circle(&self) -> Circle {
        Circle::new(self.center, 10.0)
    }
}

pub struct Road {
    pub i1: IntersectionID,
    pub i2: IntersectionID,
}

pub struct Building {
    top_left: Pt2D,
}

impl Model {
    pub fn new() -> Model {
        Model {
            intersections: BTreeMap::new(),
            roads: Vec::new(),
            buildings: Vec::new(),
        }
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        g.clear(Color::WHITE);

        for r in &self.roads {
            g.draw_line(
                Color::BLACK,
                5.0,
                &Line::new(
                    self.intersections[&r.i1].center,
                    self.intersections[&r.i2].center,
                ),
            );
        }

        for i in self.intersections.values() {
            g.draw_circle(Color::RED, &i.circle());
        }

        for b in &self.buildings {
            g.draw_rectangle(Color::BLUE, [b.top_left.x(), b.top_left.y(), 5.0, 5.0]);
        }
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
        // TODO No duplicates
        self.roads.push(Road { i1, i2 });
    }
}
