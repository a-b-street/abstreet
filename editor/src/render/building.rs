use crate::colors::ColorScheme;
use crate::objects::{Ctx, ID};
use crate::render::{RenderOptions, Renderable};
use dimensioned::si;
use ezgui::{Color, Drawable, GfxCtx, Prerender};
use geom::{Bounds, Line, Polygon, Pt2D};
use map_model::{Building, BuildingID, LANE_THICKNESS};

pub struct DrawBuilding {
    pub id: BuildingID,
    pub fill_polygon: Polygon,
    front_path: Polygon,

    default_draw: Drawable,
}

impl DrawBuilding {
    pub fn new(bldg: &Building, cs: &ColorScheme, prerender: &Prerender) -> DrawBuilding {
        // Trim the front path line away from the sidewalk's center line, so that it doesn't
        // overlap. For now, this cleanup is visual; it doesn't belong in the map_model layer.
        let mut front_path_line = bldg.front_path.line.clone();
        let len = front_path_line.length();
        let trim_back = LANE_THICKNESS / 2.0 * si::M;
        if len > trim_back {
            front_path_line = Line::new(
                front_path_line.pt1(),
                front_path_line.dist_along(len - trim_back),
            );
        }
        let fill_polygon = Polygon::new(&bldg.points);
        let front_path = front_path_line.make_polygons(1.0);

        let default_draw = prerender.upload(vec![
            (
                cs.get_def("building", Color::rgba_f(0.7, 0.7, 0.7, 0.8)),
                &fill_polygon,
            ),
            (cs.get_def("building path", Color::grey(0.6)), &front_path),
        ]);

        DrawBuilding {
            id: bldg.id,
            fill_polygon,
            front_path,
            default_draw,
        }
    }
}

impl Renderable for DrawBuilding {
    fn get_id(&self) -> ID {
        ID::Building(self.id)
    }

    fn draw(&self, g: &mut GfxCtx, opts: RenderOptions, ctx: &Ctx) {
        // Buildings look better without boundaries, actually
        //g.draw_polygon(ctx.cs.get_def("building boundary", Color::rgb(0, 100, 0)), &self.boundary_polygon);

        if let Some(c) = opts.color {
            g.draw_polygon_batch(vec![
                (c, &self.fill_polygon),
                (ctx.cs.get("building path"), &self.front_path),
            ]);
        } else {
            g.redraw(&self.default_draw);
        }
    }

    fn get_bounds(&self) -> Bounds {
        let mut b = self.fill_polygon.get_bounds();
        b.union(self.front_path.get_bounds());
        b
    }

    fn contains_pt(&self, pt: Pt2D) -> bool {
        self.fill_polygon.contains_pt(pt)
    }
}
