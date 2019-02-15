use crate::colors::ColorScheme;
use crate::objects::{DrawCtx, ID};
use crate::render::{RenderOptions, Renderable, BIG_ARROW_THICKNESS};
use ezgui::{Color, Drawable, GfxCtx, Prerender};
use geom::{Bounds, Polygon, Pt2D};
use map_model::{Map, Road, RoadID, LANE_THICKNESS};

pub struct DrawRoad {
    pub id: RoadID,
    // TODO don't even store Bounds
    bounds: Bounds,
    zorder: isize,

    draw_center_line: Drawable,
}

impl DrawRoad {
    pub fn new(r: &Road, cs: &ColorScheme, prerender: &Prerender) -> (DrawRoad, Polygon) {
        // TODO Should be a less tedious way to do this
        let width_right = (r.children_forwards.len() as f64) * LANE_THICKNESS;
        let width_left = (r.children_backwards.len() as f64) * LANE_THICKNESS;
        let total_width = width_right + width_left;
        let thick = if width_right >= width_left {
            r.center_pts
                .shift_right((width_right - width_left) / 2.0)
                .make_polygons(total_width)
        } else {
            r.center_pts
                .shift_left((width_left - width_right) / 2.0)
                .make_polygons(total_width)
        };

        (
            DrawRoad {
                id: r.id,
                bounds: thick.get_bounds(),
                zorder: r.get_zorder(),
                draw_center_line: prerender.upload(vec![(
                    cs.get_def("road center line", Color::YELLOW),
                    r.center_pts.make_polygons(BIG_ARROW_THICKNESS),
                )]),
            },
            thick,
        )
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
