use dimensioned::si;
use ezgui::{Color, GfxCtx};
use geom::{Bounds, Circle, GPSBounds, PolyLine, Polygon, Pt2D};
use kml::ExtraShape;
use objects::{Ctx, ID};
use render::{RenderOptions, Renderable, EXTRA_SHAPE_POINT_RADIUS, EXTRA_SHAPE_THICKNESS};
use std::collections::BTreeMap;
use std::fmt;

#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub struct ExtraShapeID(pub usize);

impl fmt::Display for ExtraShapeID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ExtraShapeID({0})", self.0)
    }
}

#[derive(Debug)]
enum Shape {
    Polygon(Polygon),
    Circle(Circle),
}

#[derive(Debug)]
pub struct DrawExtraShape {
    pub id: ExtraShapeID,
    shape: Shape,
    pub attributes: BTreeMap<String, String>,
}

impl DrawExtraShape {
    pub fn new(id: ExtraShapeID, s: ExtraShape, gps_bounds: &GPSBounds) -> Option<DrawExtraShape> {
        Some(DrawExtraShape {
            id: id,
            shape: if s.points.len() == 1 {
                Shape::Circle(Circle::new(
                    Pt2D::from_gps(s.points[0], gps_bounds)?,
                    EXTRA_SHAPE_POINT_RADIUS,
                ))
            } else {
                let width = get_sidewalk_width(&s.attributes)
                    .unwrap_or(EXTRA_SHAPE_THICKNESS * si::M)
                    .value_unsafe;
                let mut pts: Vec<Pt2D> = Vec::new();
                for pt in s.points.into_iter() {
                    pts.push(Pt2D::from_gps(pt, gps_bounds)?);
                }
                if let Some(p) = PolyLine::new(pts).make_polygons(width) {
                    Shape::Polygon(p)
                } else {
                    warn!(
                        "Discarding ExtraShape because its geometry was broken: {:?}",
                        s.attributes
                    );
                    return None;
                }
            },
            attributes: s.attributes,
        })
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

    fn draw(&self, g: &mut GfxCtx, opts: RenderOptions, ctx: Ctx) {
        let color = opts.color.unwrap_or(ctx.cs.get("extra shape", Color::CYAN));
        match self.shape {
            Shape::Polygon(ref p) => g.draw_polygon(color, &p),
            Shape::Circle(ref c) => g.draw_circle(color, c),
        }
    }

    fn get_bounds(&self) -> Bounds {
        match self.shape {
            Shape::Polygon(ref p) => p.get_bounds(),
            Shape::Circle(ref c) => c.get_bounds(),
        }
    }

    fn contains_pt(&self, pt: Pt2D) -> bool {
        match self.shape {
            Shape::Polygon(ref p) => p.contains_pt(pt),
            Shape::Circle(ref c) => c.contains_pt(pt),
        }
    }
}

// See https://www.seattle.gov/Documents/Departments/SDOT/GIS/Sidewalks_OD.pdf
fn get_sidewalk_width(attribs: &BTreeMap<String, String>) -> Option<si::Meter<f64>> {
    let meters_per_inch = 0.0254;
    let base_width = attribs
        .get("SW_WIDTH")
        .and_then(|s| s.parse::<f64>().ok())
        .map(|inches| inches * meters_per_inch * si::M)?;
    let filler_width = attribs
        .get("FILLERWID")
        .and_then(|s| s.parse::<f64>().ok())
        .map(|inches| inches * meters_per_inch * si::M)?;
    Some(base_width + filler_width)
}
