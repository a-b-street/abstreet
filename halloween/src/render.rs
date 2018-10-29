use aabb_quadtree::geom::{Point, Rect};
use aabb_quadtree::QuadTree;
use ezgui::{Color, GfxCtx};
use geom::{Bounds, Line, Polygon, Pt2D};
use map_model::{Building, BuildingID, Map, Road, RoadID, LANE_THICKNESS};

// black
const BACKGROUND: Color = Color([0.0, 0.0, 0.0, 1.0]);
// light orange
const ROAD: Color = Color([1.0, 154.0 / 255.0, 0.0, 1.0]);
// purple
const BUILDING: Color = Color([136.0 / 255.0, 30.0 / 255.0, 228.0 / 255.0, 1.0]);
// dark orange / red
const PATH: Color = Color([247.0 / 255.0, 95.0 / 255.0, 28.0 / 255.0, 1.0]);

const LINE_WIDTH: f64 = 1.0;

pub struct DrawMap {
    roads: Vec<DrawRoad>,
    buildings: Vec<DrawBuilding>,

    road_quadtree: QuadTree<RoadID>,
    bldg_quadtree: QuadTree<BuildingID>,
}

impl DrawMap {
    pub fn new(map: Map) -> DrawMap {
        // TODO This stuff is common!
        let bounds = map.get_bounds();
        let map_bbox = Rect {
            top_left: Point { x: 0.0, y: 0.0 },
            bottom_right: Point {
                x: bounds.max_x as f32,
                y: bounds.max_y as f32,
            },
        };

        let roads: Vec<DrawRoad> = map.all_roads().iter().map(|r| DrawRoad::new(r)).collect();
        let buildings: Vec<DrawBuilding> = map
            .all_buildings()
            .iter()
            .map(|b| DrawBuilding::new(b))
            .collect();

        // TODO This is a bit boilerplateish
        let mut road_quadtree = QuadTree::default(map_bbox);
        for r in &roads {
            road_quadtree.insert_with_box(r.id, r.get_bounds().as_bbox());
        }
        let mut bldg_quadtree = QuadTree::default(map_bbox);
        for b in &buildings {
            bldg_quadtree.insert_with_box(b.id, b.get_bounds().as_bbox());
        }

        DrawMap {
            roads,
            buildings,
            road_quadtree,
            bldg_quadtree,
        }
    }

    pub fn draw(&self, g: &mut GfxCtx, timer: f64, screen_bbox: Rect) {
        g.clear(BACKGROUND);

        for &(id, _, _) in &self.road_quadtree.query(screen_bbox) {
            self.roads[id.0].draw(g);
        }
        for &(id, _, _) in &self.bldg_quadtree.query(screen_bbox) {
            self.buildings[id.0].draw(g, timer);
        }
    }
}

struct DrawRoad {
    id: RoadID,
    polygon: Polygon,
}

impl DrawRoad {
    fn new(r: &Road) -> DrawRoad {
        // TODO Should shift if the number of children is uneven
        let num_lanes = r.children_forwards.len() + r.children_backwards.len();
        DrawRoad {
            id: r.id,
            polygon: r
                .center_pts
                .make_polygons_blindly(LANE_THICKNESS * (num_lanes as f64)),
        }
    }

    fn draw(&self, g: &mut GfxCtx) {
        g.draw_polygon(ROAD, &self.polygon);
    }

    fn get_bounds(&self) -> Bounds {
        self.polygon.get_bounds()
    }
}

struct DrawBuilding {
    id: BuildingID,
    // The points when the line is full.
    polygon: Polygon,
    // pt1 is fixed, to the road
    line: Line,
}

impl DrawBuilding {
    fn new(b: &Building) -> DrawBuilding {
        DrawBuilding {
            id: b.id,
            polygon: Polygon::new(&b.points),
            line: b.front_path.line.reverse(),
        }
    }

    fn draw(&self, g: &mut GfxCtx, timer: f64) {
        let percent = timer;
        let dx = self.line.pt2().x() - self.line.pt1().x();
        let dy = self.line.pt2().y() - self.line.pt1().y();

        // TODO or modify g's ctx
        g.draw_polygon(
            BUILDING,
            &self
                .polygon
                .translate(-1.0 * (1.0 - percent) * dx, -1.0 * (1.0 - percent) * dy),
        );

        let new_line = Line::new(
            self.line.pt1(),
            Pt2D::new(
                self.line.pt1().x() + percent * dx,
                self.line.pt1().y() + percent * dy,
            ),
        );
        g.draw_rounded_line(PATH, LINE_WIDTH, &new_line);
    }

    fn get_bounds(&self) -> Bounds {
        // The bbox only shrinks; the original position is the worst case.
        let mut b = self.polygon.get_bounds();
        b.update_pt(self.line.pt1());
        b.update_pt(self.line.pt2());
        b
    }
}
