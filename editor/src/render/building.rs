// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use crate::objects::{Ctx, ID};
use crate::render::{RenderOptions, Renderable};
use dimensioned::si;
use ezgui::{Color, GfxCtx};
use geom::{Bounds, Line, Polygon, Pt2D};
use map_model::{Building, BuildingID, LANE_THICKNESS};

pub struct DrawBuilding {
    pub id: BuildingID,
    pub fill_polygon: Polygon,
    front_path: Line,
}

impl DrawBuilding {
    pub fn new(bldg: &Building) -> DrawBuilding {
        // Trim the front path line away from the sidewalk's center line, so that it doesn't
        // overlap. For now, this cleanup is visual; it doesn't belong in the map_model layer.
        let mut front_path = bldg.front_path.line.clone();
        let len = front_path.length();
        let trim_back = LANE_THICKNESS / 2.0 * si::M;
        if len > trim_back {
            front_path = Line::new(front_path.pt1(), front_path.dist_along(len - trim_back));
        }

        DrawBuilding {
            id: bldg.id,
            front_path,
            fill_polygon: Polygon::new(&bldg.points),
        }
    }
}

impl Renderable for DrawBuilding {
    fn get_id(&self) -> ID {
        ID::Building(self.id)
    }

    fn draw(&self, g: &mut GfxCtx, opts: RenderOptions, ctx: &mut Ctx) {
        // Buildings look better without boundaries, actually
        //g.draw_polygon(ctx.cs.get_def("building boundary", Color::rgb(0, 100, 0)), &self.boundary_polygon);
        g.draw_polygon(
            opts.color.unwrap_or_else(|| {
                ctx.cs
                    .get_def("building", Color::rgba_f(0.7, 0.7, 0.7, 0.8))
            }),
            &self.fill_polygon,
        );

        g.draw_line(
            ctx.cs.get_def("building path", Color::grey(0.6)),
            1.0,
            &self.front_path,
        );
    }

    fn get_bounds(&self) -> Bounds {
        let mut b = self.fill_polygon.get_bounds();
        b.update(self.front_path.pt1());
        b.update(self.front_path.pt2());
        b
    }

    fn contains_pt(&self, pt: Pt2D) -> bool {
        self.fill_polygon.contains_pt(pt)
    }
}
