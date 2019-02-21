use crate::colors::ColorScheme;
use crate::objects::{DrawCtx, ID};
use crate::render::{RenderOptions, Renderable, BIG_ARROW_THICKNESS};
use ezgui::{Color, Drawable, GfxCtx, Prerender};
use geom::{Bounds, Pt2D};
use map_model::{Map, Road, RoadID};

pub struct DrawRoad {
    pub id: RoadID,
    // TODO don't even store Bounds
    bounds: Bounds,
    zorder: isize,

    draw_center_line: Drawable,
}

impl DrawRoad {
    pub fn new(r: &Road, cs: &ColorScheme, prerender: &Prerender) -> DrawRoad {
        DrawRoad {
            id: r.id,
            // TODO Urgh, gotta pass timer in
            bounds: r.get_thick_polygon().unwrap().get_bounds(),
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

    fn get_bounds(&self, _: &Map) -> Bounds {
        self.bounds.clone()
    }

    // Can't select these
    fn contains_pt(&self, _: Pt2D, _: &Map) -> bool {
        false
    }

    fn get_zorder(&self) -> isize {
        self.zorder
    }
}
