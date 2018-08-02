// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use aabb_quadtree::geom::Rect;
use ezgui::GfxCtx;
use geom::PolyLine;
use graphics;
use graphics::math::Vec2d;
use graphics::types::Color;
use map_model;
use map_model::geometry;
use map_model::{BuildingID, Map};
use render::PARCEL_BOUNDARY_THICKNESS;
use std::f64;

#[derive(Debug)]
pub struct DrawBuilding {
    pub id: BuildingID,
    // TODO should just have one. use graphics::Line for now.
    boundary_polygons: Vec<Vec<Vec2d>>,
    pub fill_polygon: Vec<Vec2d>,
    front_path: Option<[f64; 4]>,
}

impl DrawBuilding {
    pub fn new(bldg: &map_model::Building) -> DrawBuilding {
        let pts: Vec<Vec2d> = bldg.points.iter().map(|pt| [pt.x(), pt.y()]).collect();
        DrawBuilding {
            id: bldg.id,
            // TODO ideally start the path on a side of the building
            front_path: bldg.front_path
                .as_ref()
                .map(|l| [l.pt1().x(), l.pt1().y(), l.pt2().x(), l.pt2().y()]),
            fill_polygon: pts,
            boundary_polygons: PolyLine::new(bldg.points.clone())
                .make_polygons_blindly(PARCEL_BOUNDARY_THICKNESS),
        }
    }

    pub fn draw(
        &self,
        g: &mut GfxCtx,
        fill_color: Color,
        path_color: Color,
        boundary_color: Color,
    ) {
        if let Some(line) = self.front_path {
            // TODO tune width
            g.draw_line(&graphics::Line::new_round(path_color, 1.0), line);
        }

        for p in &self.boundary_polygons {
            g.draw_polygon(boundary_color, p);
        }
        // TODO the triangulation seems messed up. ><
        g.draw_polygon(fill_color, &self.fill_polygon);
    }

    pub fn contains_pt(&self, x: f64, y: f64) -> bool {
        geometry::point_in_polygon(x, y, &self.fill_polygon)
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
        let mut polygons = vec![self.fill_polygon.clone()];
        if let Some(line) = self.front_path {
            polygons.push(vec![[line[0], line[1]], [line[2], line[3]]]);
        }
        geometry::get_bbox_for_polygons(&polygons)
    }
}
