// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use aabb_quadtree::geom::Rect;
use colors::Colors;
use ezgui::GfxCtx;
use geom::{Line, PolyLine, Polygon, Pt2D};
use map_model::{Building, BuildingID, Map};
use objects::{Ctx, ID};
use render::{get_bbox, RenderOptions, Renderable, BUILDING_BOUNDARY_THICKNESS};

#[derive(Debug)]
pub struct DrawBuilding {
    pub id: BuildingID,
    // TODO bit wasteful to keep both
    boundary_polygon: Polygon,
    pub fill_polygon: Polygon,
    front_path: Line,
}

impl DrawBuilding {
    pub fn new(bldg: &Building) -> DrawBuilding {
        DrawBuilding {
            id: bldg.id,
            front_path: bldg.front_path.line.clone(),
            fill_polygon: Polygon::new(&bldg.points),
            boundary_polygon: PolyLine::new(bldg.points.clone())
                .make_polygons_blindly(BUILDING_BOUNDARY_THICKNESS),
        }
    }
}

impl Renderable for DrawBuilding {
    fn get_id(&self) -> ID {
        ID::Building(self.id)
    }

    fn draw(&self, g: &mut GfxCtx, opts: RenderOptions, ctx: Ctx) {
        // Buildings look better without boundaries, actually
        //g.draw_polygon(ctx.cs.get(Colors::BuildingBoundary), &self.boundary_polygon);
        g.draw_polygon(
            opts.color.unwrap_or(ctx.cs.get(Colors::Building)),
            &self.fill_polygon,
        );

        // TODO tune width
        g.draw_rounded_line(ctx.cs.get(Colors::BuildingPath), 1.0, &self.front_path);
    }

    fn get_bbox(&self) -> Rect {
        let mut b = self.fill_polygon.get_bounds();
        b.update_pt(self.front_path.pt1());
        b.update_pt(self.front_path.pt2());
        get_bbox(&b)
    }

    fn contains_pt(&self, pt: Pt2D) -> bool {
        self.fill_polygon.contains_pt(pt)
    }

    fn tooltip_lines(&self, map: &Map) -> Vec<String> {
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
}
