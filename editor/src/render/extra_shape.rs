use crate::objects::{DrawCtx, ID};
use crate::render::{RenderOptions, Renderable, EXTRA_SHAPE_POINT_RADIUS, EXTRA_SHAPE_THICKNESS};
use ezgui::{Color, GfxCtx};
use geom::{Bounds, Circle, Distance, FindClosest, GPSBounds, PolyLine, Polygon, Pt2D};
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

enum Shape {
    Polygon(Polygon),
    Circle(Circle),
}

pub struct DrawExtraShape {
    pub id: ExtraShapeID,
    shape: Shape,
    pub attributes: BTreeMap<String, String>,
    pub road: Option<DirectedRoadID>,
}

impl DrawExtraShape {
    pub fn new(
        id: ExtraShapeID,
        s: ExtraShape,
        gps_bounds: &GPSBounds,
        closest: &FindClosest<DirectedRoadID>,
    ) -> Option<DrawExtraShape> {
        let mut pts: Vec<Pt2D> = Vec::new();
        for pt in s.points.into_iter() {
            pts.push(Pt2D::from_gps(pt, gps_bounds)?);
        }

        if pts.len() == 1 {
            Some(DrawExtraShape {
                id,
                shape: Shape::Circle(Circle::new(pts[0], EXTRA_SHAPE_POINT_RADIUS)),
                attributes: s.attributes,
                road: None,
            })
        } else {
            let width = get_sidewalk_width(&s.attributes).unwrap_or(EXTRA_SHAPE_THICKNESS);
            let pl = PolyLine::new(pts);
            // The blockface line endpoints will be close to other roads, so match based on the
            // middle of the blockface.
            // TODO Long blockfaces sometimes cover two roads. Should maybe find ALL matches within
            // the threshold distance?
            let road = closest
                .closest_pt(pl.middle(), LANE_THICKNESS * 5.0)
                .map(|(r, _)| r);
            Some(DrawExtraShape {
                id,
                shape: Shape::Polygon(pl.make_polygons(width)),
                attributes: s.attributes,
                road,
            })
        }
    }

    pub fn center(&self) -> Pt2D {
        match self.shape {
            Shape::Polygon(ref p) => Pt2D::center(&p.points()),
            Shape::Circle(ref c) => c.center,
        }
    }
}

impl Renderable for DrawExtraShape {
    fn get_id(&self) -> ID {
        ID::ExtraShape(self.id)
    }

    fn draw(&self, g: &mut GfxCtx, opts: RenderOptions, ctx: &DrawCtx) {
        let color = opts
            .color
            .unwrap_or_else(|| ctx.cs.get_def("extra shape", Color::CYAN));
        match self.shape {
            Shape::Polygon(ref p) => g.draw_polygon(color, &p),
            Shape::Circle(ref c) => g.draw_circle(color, c),
        }
    }

    fn get_bounds(&self, _: &Map) -> Bounds {
        match self.shape {
            Shape::Polygon(ref p) => p.get_bounds(),
            Shape::Circle(ref c) => c.get_bounds(),
        }
    }

    fn contains_pt(&self, pt: Pt2D, _: &Map) -> bool {
        match self.shape {
            Shape::Polygon(ref p) => p.contains_pt(pt),
            Shape::Circle(ref c) => c.contains_pt(pt),
        }
    }
}

// See https://www.seattle.gov/Documents/Departments/SDOT/GIS/Sidewalks_OD.pdf
fn get_sidewalk_width(attribs: &BTreeMap<String, String>) -> Option<Distance> {
    let base_width = attribs
        .get("SW_WIDTH")
        .and_then(|s| s.parse::<f64>().ok())
        .map(Distance::inches)?;
    let filler_width = attribs
        .get("FILLERWID")
        .and_then(|s| s.parse::<f64>().ok())
        .map(Distance::inches)?;
    Some(base_width + filler_width)
}
