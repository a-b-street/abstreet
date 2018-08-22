// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use aabb_quadtree::geom::Rect;
use ezgui::GfxCtx;
use geom::{PolyLine, Polygon, Pt2D};
use graphics;
use graphics::types::Color;
use map_model::{Building, BuildingID, Map};
use render::{get_bbox, BUILDING_BOUNDARY_THICKNESS};
use std::f64;

#[derive(Debug)]
pub struct DrawBuilding {
    pub id: BuildingID,
    // TODO should just have one. use graphics::Line for now.
    boundary_polygon: Polygon,
    pub fill_polygon: Polygon,
    front_path: [f64; 4],
}

impl DrawBuilding {
    pub fn new(bldg: &Building) -> DrawBuilding {
        DrawBuilding {
            id: bldg.id,
            front_path: {
                let l = &bldg.front_path;
                [l.pt1().x(), l.pt1().y(), l.pt2().x(), l.pt2().y()]
            },
            fill_polygon: Polygon::new(&bldg.points),
            boundary_polygon: PolyLine::new(bldg.points.clone())
                .make_polygons_blindly(BUILDING_BOUNDARY_THICKNESS),
        }
    }

    pub fn draw(
        &self,
        g: &mut GfxCtx,
        fill_color: Color,
        path_color: Color,
        boundary_color: Color,
    ) {
        // TODO tune width
        g.draw_line(&graphics::Line::new_round(path_color, 1.0), self.front_path);

        g.draw_polygon(boundary_color, &self.boundary_polygon);
        g.draw_polygon(fill_color, &self.fill_polygon);
    }

    pub fn contains_pt(&self, pt: Pt2D) -> bool {
        self.fill_polygon.contains_pt(pt)
    }

    pub fn tooltip_lines(&self, map: &Map) -> Vec<String> {
        let b = map.get_b(self.id);
        let mut lines = vec![format!(
            "Building #{:?} (from OSM way {})",
            self.id, b.osm_way_id
        )];
        for (k, v) in &b.osm_tags {
            lines.push(format!("{} = {}", k, v));
        }
        lines
    }

    pub fn get_bbox(&self) -> Rect {
        let mut b = self.fill_polygon.get_bounds();
        b.update(self.front_path[0], self.front_path[1]);
        b.update(self.front_path[2], self.front_path[3]);
        get_bbox(&b)
    }
}
