use crate::colors::ColorScheme;
use crate::objects::{DrawCtx, ID};
use crate::render::{RenderOptions, Renderable};
use ezgui::{Color, GfxCtx};
use geom::{Bounds, Distance, Line, Polygon, Pt2D};
use map_model::{Building, BuildingID, BuildingType, Map, LANE_THICKNESS};

pub struct DrawBuilding {
    pub id: BuildingID,
    // TODO Return from new() and don't even store this.
    bounds: Bounds,
}

impl DrawBuilding {
    pub fn new(bldg: &Building, cs: &ColorScheme) -> (DrawBuilding, Vec<(Color, Polygon)>) {
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

        let mut bounds = bldg.polygon.get_bounds();
        bounds.union(front_path.get_bounds());

        let default_draw = vec![
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
                bldg.polygon.clone(),
            ),
            (cs.get_def("building path", Color::grey(0.6)), front_path),
        ];

        (
            DrawBuilding {
                id: bldg.id,
                bounds,
            },
            default_draw,
        )
    }
}

impl Renderable for DrawBuilding {
    fn get_id(&self) -> ID {
        ID::Building(self.id)
    }

    fn draw(&self, g: &mut GfxCtx, opts: RenderOptions, ctx: &DrawCtx) {
        if let Some(c) = opts.color {
            g.draw_polygon(c, &ctx.map.get_b(self.id).polygon);
        }
    }

    fn get_bounds(&self, _: &Map) -> Bounds {
        self.bounds.clone()
    }

    fn contains_pt(&self, pt: Pt2D, map: &Map) -> bool {
        map.get_b(self.id).polygon.contains_pt(pt)
    }
}
