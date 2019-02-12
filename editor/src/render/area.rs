use crate::colors::ColorScheme;
use crate::objects::{DrawCtx, ID};
use crate::render::{RenderOptions, Renderable};
use ezgui::{Color, Drawable, GfxCtx, Prerender};
use geom::{Bounds, Polygon, Pt2D};
use map_model::{Area, AreaID, AreaType, Map};

pub struct DrawArea {
    pub id: AreaID,
    fill_polygon: Polygon,

    draw_default: Drawable,
}

impl DrawArea {
    pub fn new(area: &Area, cs: &ColorScheme, prerender: &Prerender) -> DrawArea {
        let fill_polygon = area.get_polygon();
        let draw_default = prerender.upload_borrowed(vec![(
            match area.area_type {
                AreaType::Park => cs.get_def("park area", Color::rgb(200, 250, 204)),
                AreaType::Water => cs.get_def("water area", Color::rgb(170, 211, 223)),
            },
            &fill_polygon,
        )]);

        DrawArea {
            id: area.id,
            fill_polygon,
            draw_default,
        }
    }
}

impl Renderable for DrawArea {
    fn get_id(&self) -> ID {
        ID::Area(self.id)
    }

    fn draw(&self, g: &mut GfxCtx, opts: RenderOptions, _ctx: &DrawCtx) {
        if let Some(color) = opts.color {
            g.draw_polygon(color, &self.fill_polygon);
        } else {
            g.redraw(&self.draw_default);
        }
    }

    fn get_bounds(&self, _: &Map) -> Bounds {
        self.fill_polygon.get_bounds()
    }

    fn contains_pt(&self, pt: Pt2D, _: &Map) -> bool {
        self.fill_polygon.contains_pt(pt)
    }
}
