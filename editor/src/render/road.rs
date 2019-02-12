use crate::colors::ColorScheme;
use crate::objects::{DrawCtx, ID};
use crate::render::{RenderOptions, Renderable, BIG_ARROW_THICKNESS, MIN_ZOOM_FOR_MARKINGS};
use ezgui::{Color, Drawable, GfxCtx, Prerender};
use geom::{Bounds, Polygon, Pt2D};
use map_model::{Road, RoadID, LANE_THICKNESS};

pub struct DrawRoad {
    pub id: RoadID,
    polygon: Polygon,
    zorder: isize,

    draw_unzoomed_band: Drawable,
    draw_center_line: Drawable,
}

impl DrawRoad {
    pub fn new(r: &Road, cs: &ColorScheme, prerender: &Prerender) -> DrawRoad {
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
        let draw_unzoomed_band = prerender.upload_borrowed(vec![(
            cs.get_def("unzoomed road band", Color::BLACK),
            &thick,
        )]);

        DrawRoad {
            id: r.id,
            polygon: thick,
            zorder: r.get_zorder(),
            draw_unzoomed_band,
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

    fn draw(&self, g: &mut GfxCtx, opts: RenderOptions, _ctx: &DrawCtx) {
        if g.canvas.cam_zoom >= MIN_ZOOM_FOR_MARKINGS || opts.show_all_detail {
            g.redraw(&self.draw_center_line);
        } else if let Some(color) = opts.color {
            g.draw_polygon(color, &self.polygon);
        } else {
            g.redraw(&self.draw_unzoomed_band);
        }
    }

    fn get_bounds(&self) -> Bounds {
        self.polygon.get_bounds()
    }

    fn contains_pt(&self, pt: Pt2D) -> bool {
        self.polygon.contains_pt(pt)
    }

    fn get_zorder(&self) -> isize {
        self.zorder
    }
}
