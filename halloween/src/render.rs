use ezgui::GfxCtx;
use geom::{Line, Polygon, Pt2D};
use map_model::{Building, Map, Road, LANE_THICKNESS};

const WHITE: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
const BLACK: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
const RED: [f32; 4] = [1.0, 0.0, 0.0, 1.0];
const BLUE: [f32; 4] = [0.0, 0.0, 1.0, 1.0];

const LINE_WIDTH: f64 = 1.0;

pub struct DrawMap {
    roads: Vec<DrawRoad>,
    buildings: Vec<DrawBuilding>,
}

impl DrawMap {
    pub fn new(map: Map) -> DrawMap {
        DrawMap {
            roads: map.all_roads().iter().map(|r| DrawRoad::new(r)).collect(),
            buildings: map
                .all_buildings()
                .iter()
                .map(|b| DrawBuilding::new(b))
                .collect(),
        }
    }

    pub fn draw(&self, g: &mut GfxCtx, timer: f64) {
        g.clear(WHITE);
        // TODO no pruning yet
        for r in &self.roads {
            r.draw(g);
        }
        for b in &self.buildings {
            b.draw(g, timer);
        }
    }
}

struct DrawRoad {
    polygon: Polygon,
}

impl DrawRoad {
    fn new(r: &Road) -> DrawRoad {
        // TODO Should shift if the number of children is uneven
        let num_lanes = r.children_forwards.len() + r.children_backwards.len();
        DrawRoad {
            polygon: r
                .center_pts
                .make_polygons_blindly(LANE_THICKNESS * (num_lanes as f64)),
        }
    }

    fn draw(&self, g: &mut GfxCtx) {
        g.draw_polygon(BLACK, &self.polygon);
    }
}

struct DrawBuilding {
    polygon: Polygon,
    // pt1 is fixed, to the road
    line: Line,
}

impl DrawBuilding {
    fn new(b: &Building) -> DrawBuilding {
        DrawBuilding {
            polygon: Polygon::new(&b.points),
            line: b.front_path.line.reverse(),
        }
    }

    fn draw(&self, g: &mut GfxCtx, timer: f64) {
        g.draw_polygon(RED, &self.polygon);
        let percent = timer;
        let new_line = Line::new(
            self.line.pt1(),
            Pt2D::new(
                self.line.pt1().x() + percent * (self.line.pt2().x() - self.line.pt1().x()),
                self.line.pt1().y() + percent * (self.line.pt2().y() - self.line.pt1().y()),
            ),
        );
        g.draw_rounded_line(BLUE, LINE_WIDTH, &new_line);
    }
}
