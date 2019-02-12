use crate::colors::ColorScheme;
use crate::objects::{DrawCtx, ID};
use crate::render::{RenderOptions, Renderable};
use ezgui::{Color, Drawable, GfxCtx, Prerender};
use geom::{Bounds, Distance, Line, Polygon, Pt2D};
use map_model::{Building, BuildingID, BuildingType, Map, LANE_THICKNESS};

pub struct DrawBuilding {
    pub id: BuildingID,
    front_path: Polygon,

    default_draw: Drawable,
}

impl DrawBuilding {
    pub fn new(bldg: &Building, cs: &ColorScheme, prerender: &Prerender) -> DrawBuilding {
        // Trim the front path line away from the sidewalk's center line, so that it doesn't
        // overlap. For now, this cleanup is visual; it doesn't belong in the map_model layer.
        let mut front_path_line = bldg.front_path.line.clone();
        let len = front_path_line.length();
        let trim_back = LANE_THICKNESS / 2.0;
        if len > trim_back && len - trim_back > geom::EPSILON_DIST {
            front_path_line = Line::new(
                front_path_line.pt1(),
                front_path_line.dist_along(len - trim_back),
            );
        }
        let front_path = front_path_line.make_polygons(Distance::meters(1.0));

        let default_draw = prerender.upload_borrowed(vec![
            (
                match bldg.building_type {
                    BuildingType::Residence => {
                        cs.get_def("residential building", Color::rgb(218, 165, 32))
                    }
                    BuildingType::Business => {
                        cs.get_def("business building", Color::rgb(210, 105, 30))
                    }
                    BuildingType::Unknown => {
                        cs.get_def("unknown building", Color::rgb_f(0.7, 0.7, 0.7))
                    }
                },
                &bldg.polygon,
            ),
            (cs.get_def("building path", Color::grey(0.6)), &front_path),
        ]);

        DrawBuilding {
            id: bldg.id,
            front_path,
            default_draw,
        }
    }
}

impl Renderable for DrawBuilding {
    fn get_id(&self) -> ID {
        ID::Building(self.id)
    }

    fn draw(&self, g: &mut GfxCtx, opts: RenderOptions, ctx: &DrawCtx) {
        if let Some(c) = opts.color {
            g.draw_polygon_batch(vec![
                (c, &ctx.map.get_b(self.id).polygon),
                (ctx.cs.get("building path"), &self.front_path),
            ]);
        } else {
            g.redraw(&self.default_draw);
        }
    }

    fn get_bounds(&self, map: &Map) -> Bounds {
        let mut b = map.get_b(self.id).polygon.get_bounds();
        b.union(self.front_path.get_bounds());
        b
    }

    fn contains_pt(&self, pt: Pt2D, map: &Map) -> bool {
        map.get_b(self.id).polygon.contains_pt(pt)
    }
}
