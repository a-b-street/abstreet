use crate::colors::ColorScheme;
use crate::objects::{Ctx, ID};
use crate::render::{RenderOptions, Renderable, PARCEL_BOUNDARY_THICKNESS};
use ezgui::{Color, Drawable, GfxCtx, Prerender};
use geom::{Bounds, PolyLine, Polygon, Pt2D};
use map_model::{Parcel, ParcelID};

const COLORS: [Color; 14] = [
    // TODO these are awful choices
    Color::rgba_f(1.0, 1.0, 0.0, 1.0),
    Color::rgba_f(1.0, 0.0, 1.0, 1.0),
    Color::rgba_f(0.0, 1.0, 1.0, 1.0),
    Color::rgba_f(0.5, 0.2, 0.7, 1.0),
    Color::rgba_f(0.5, 0.5, 0.0, 0.5),
    Color::rgba_f(0.5, 0.0, 0.5, 0.5),
    Color::rgba_f(0.0, 0.5, 0.5, 0.5),
    Color::rgba_f(0.0, 0.0, 0.5, 0.5),
    Color::rgba_f(0.3, 0.2, 0.5, 0.5),
    Color::rgba_f(0.4, 0.2, 0.5, 0.5),
    Color::rgba_f(0.5, 0.2, 0.5, 0.5),
    Color::rgba_f(0.6, 0.2, 0.5, 0.5),
    Color::rgba_f(0.7, 0.2, 0.5, 0.5),
    Color::rgba_f(0.8, 0.2, 0.5, 0.5),
];

pub struct DrawParcel {
    pub id: ParcelID,
    // TODO bit wasteful to keep both
    boundary_polygon: Polygon,
    pub fill_polygon: Polygon,

    default_draw: Drawable,
}

impl DrawParcel {
    pub fn new(p: &Parcel, cs: &ColorScheme, prerender: &Prerender) -> DrawParcel {
        let boundary_polygon =
            PolyLine::make_polygons_for_boundary(p.points.clone(), PARCEL_BOUNDARY_THICKNESS);
        let fill_polygon = Polygon::new(&p.points);
        let default_draw = prerender.upload_borrowed(vec![
            (COLORS[p.block % COLORS.len()], &fill_polygon),
            (
                cs.get_def("parcel boundary", Color::grey(0.3)),
                &boundary_polygon,
            ),
        ]);

        DrawParcel {
            id: p.id,
            boundary_polygon,
            fill_polygon,
            default_draw,
        }
    }
}

impl Renderable for DrawParcel {
    fn get_id(&self) -> ID {
        ID::Parcel(self.id)
    }

    fn draw(&self, g: &mut GfxCtx, opts: RenderOptions, ctx: &Ctx) {
        if let Some(color) = opts.color {
            g.draw_polygon_batch(vec![
                (color, &self.fill_polygon),
                (ctx.cs.get("parcel boundary"), &self.boundary_polygon),
            ]);
        } else {
            g.redraw(&self.default_draw);
        }
    }

    fn get_bounds(&self) -> Bounds {
        self.fill_polygon.get_bounds()
    }

    fn contains_pt(&self, pt: Pt2D) -> bool {
        self.fill_polygon.contains_pt(pt)
    }
}
