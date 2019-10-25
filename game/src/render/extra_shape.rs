use crate::helpers::{ColorScheme, ID};
use crate::render::{
    DrawCtx, DrawOptions, Renderable, EXTRA_SHAPE_POINT_RADIUS, EXTRA_SHAPE_THICKNESS,
};
use ezgui::{Color, Drawable, GeomBatch, GfxCtx, Prerender};
use geom::{Circle, FindClosest, GPSBounds, PolyLine, Polygon, Pt2D, Ring};
use kml::ExtraShape;
use map_model::{DirectedRoadID, Map, LANE_THICKNESS};
use std::collections::BTreeMap;
use std::fmt;

#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub struct ExtraShapeID(pub usize);

impl fmt::Display for ExtraShapeID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ExtraShapeID({0})", self.0)
    }
}

pub struct DrawExtraShape {
    pub id: ExtraShapeID,
    polygon: Polygon,
    draw_default: Drawable,
    pub attributes: BTreeMap<String, String>,
    pub road: Option<DirectedRoadID>,
}

impl DrawExtraShape {
    pub fn new(
        id: ExtraShapeID,
        s: ExtraShape,
        gps_bounds: &GPSBounds,
        closest: &FindClosest<DirectedRoadID>,
        prerender: &Prerender,
        cs: &ColorScheme,
    ) -> Option<DrawExtraShape> {
        let mut pts: Vec<Pt2D> = Vec::new();
        for pt in s.points.into_iter() {
            pts.push(Pt2D::from_gps(pt, gps_bounds)?);
        }

        // TODO Can we do something better?
        if pts.windows(2).any(|pair| pair[0] == pair[1]) {
            return None;
        }

        let road = closest
            .closest_pt(Pt2D::center(&pts), LANE_THICKNESS * 5.0)
            .map(|(r, _)| r);

        let polygon = if pts.len() == 1 {
            Circle::new(pts[0], EXTRA_SHAPE_POINT_RADIUS).to_polygon()
        } else if pts[0] == *pts.last().unwrap() {
            // TODO Toggle between these better
            //Polygon::new(&pts)
            Ring::new(pts).make_polygons(EXTRA_SHAPE_THICKNESS)
        } else {
            PolyLine::new(pts).make_polygons(EXTRA_SHAPE_THICKNESS)
        };
        let mut batch = GeomBatch::new();
        batch.push(
            cs.get_def("extra shape", Color::RED.alpha(0.5)),
            polygon.clone(),
        );

        Some(DrawExtraShape {
            id,
            polygon,
            draw_default: prerender.upload(batch),
            attributes: s.attributes,
            road,
        })
    }

    pub fn center(&self) -> Pt2D {
        self.polygon.center()
    }
}

impl Renderable for DrawExtraShape {
    fn get_id(&self) -> ID {
        ID::ExtraShape(self.id)
    }

    fn draw(&self, g: &mut GfxCtx, opts: &DrawOptions, _: &DrawCtx) {
        if let Some(color) = opts.color(self.get_id()) {
            g.draw_polygon(color, &self.polygon);
        } else {
            g.redraw(&self.draw_default);
        }
    }

    fn get_outline(&self, _: &Map) -> Polygon {
        // TODO This depends on the original input type
        self.polygon.clone()
    }

    fn get_zorder(&self) -> isize {
        5
    }
}
