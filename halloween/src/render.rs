use aabb_quadtree::QuadTree;
use ezgui::{Color, GfxCtx, Prerender, Text};
use geom::{Bounds, Distance, Line, Polygon, Pt2D};
use map_model::{Building, BuildingID, Map, RoadID, LANE_THICKNESS};
use viewer::World;

// black
const BACKGROUND: Color = Color::rgb_f(0.0, 0.0, 0.0);
// light orange
const ROAD: Color = Color::rgb_f(1.0, 154.0 / 255.0, 0.0);
// purple
const BUILDING: Color = Color::rgb_f(136.0 / 255.0, 30.0 / 255.0, 228.0 / 255.0);
// dark orange / red
const PATH: Color = Color::rgb_f(247.0 / 255.0, 95.0 / 255.0, 28.0 / 255.0);

const LINE_WIDTH: Distance = Distance::const_meters(1.0);

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
enum ID {
    Road(RoadID),
}

impl viewer::ObjectID for ID {
    fn zorder(&self) -> usize {
        0
    }
}

pub struct DrawMap {
    buildings: Vec<DrawBuilding>,
    bldg_quadtree: QuadTree<BuildingID>,

    world: World<ID>,
}

impl DrawMap {
    pub fn new(map: Map, prerender: &Prerender) -> DrawMap {
        let mut world = World::new(map.get_bounds());

        for r in map.all_roads() {
            // TODO Should shift if the number of children is uneven
            let num_lanes = r.children_forwards.len() + r.children_backwards.len();

            world.add_obj(
                prerender,
                ID::Road(r.id),
                r.center_pts
                    .make_polygons(LANE_THICKNESS * (num_lanes as f64)),
                ROAD,
                Text::from_line(format!("{}", r.id)),
            );
        }

        let buildings: Vec<DrawBuilding> = map
            .all_buildings()
            .iter()
            .map(|b| DrawBuilding::new(b))
            .collect();
        let mut bldg_quadtree = QuadTree::default(map.get_bounds().as_bbox());
        for b in &buildings {
            bldg_quadtree.insert_with_box(b.id, b.get_bounds().as_bbox());
        }

        DrawMap {
            world,
            buildings,
            bldg_quadtree,
        }
    }

    pub fn draw(&self, g: &mut GfxCtx, timer: f64) {
        g.clear(BACKGROUND);

        self.world.draw(g);

        for &(id, _, _) in &self.bldg_quadtree.query(g.get_screen_bounds().as_bbox()) {
            self.buildings[id.0].draw(g, timer);
        }
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

        if let Some(new_line) = Line::maybe_new(
            self.line.pt1(),
            Pt2D::new(
                self.line.pt1().x() + percent * dx,
                self.line.pt1().y() + percent * dy,
            ),
        ) {
            g.draw_rounded_line(PATH, LINE_WIDTH, &new_line);
        }
    }

    fn get_bounds(&self) -> Bounds {
        // The bbox only shrinks; the original position is the worst case.
        let mut b = self.polygon.get_bounds();
        b.update(self.line.pt1());
        b.update(self.line.pt2());
        b
    }
}
