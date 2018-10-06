use ezgui::GfxCtx;
use geom::Polygon;
use map_model::{Map, Road, RoadID, LANE_THICKNESS};

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
        g.draw_polygon([0.0, 0.0, 0.0, 1.0], &self.polygon);
    }
}

pub struct DrawMap {
    roads: Vec<DrawRoad>,
}

impl DrawMap {
    pub fn new(map: Map) -> DrawMap {
        DrawMap {
            roads: map.all_roads().iter().map(|r| DrawRoad::new(r)).collect(),
        }
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        // TODO no pruning yet
        for r in &self.roads {
            r.draw(g);
        }
    }
}
