use crate::helpers::{ColorScheme, DrawCtx, ID};
use crate::render::{RenderOptions, Renderable, BIG_ARROW_THICKNESS};
use ezgui::{Color, Drawable, GfxCtx, Prerender};
use geom::Polygon;
use map_model::{Map, Road, RoadID};

pub struct DrawRoad {
    pub id: RoadID,
    zorder: isize,

    draw_center_line: Drawable,
}

impl DrawRoad {
    pub fn new(r: &Road, cs: &ColorScheme, prerender: &Prerender) -> DrawRoad {
        DrawRoad {
            id: r.id,
            zorder: r.get_zorder(),
            draw_center_line: prerender.upload(vec![(
                cs.get_def("road center line", Color::YELLOW),
                r.center_pts.make_polygons(BIG_ARROW_THICKNESS),
            )]),
        }
    }
}

impl Renderable for DrawRoad {
    fn get_id(&self) -> ID {
        ID::Road(self.id)
    }

    fn draw(&self, g: &mut GfxCtx, _: RenderOptions, _: &DrawCtx) {
        g.redraw(&self.draw_center_line);
    }

    fn get_outline(&self, map: &Map) -> Polygon {
        map.get_r(self.id).get_thick_polygon().unwrap()
    }

    fn get_zorder(&self) -> isize {
        self.zorder
    }
}
