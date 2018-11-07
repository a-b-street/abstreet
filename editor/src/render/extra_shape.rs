use dimensioned::si;
use ezgui::{Color, GfxCtx};
use geom::{Bounds, Circle, Polygon, Pt2D};
use kml::{ExtraShape, ExtraShapeGeom, ExtraShapeID};
use map_model::Map;
use objects::{Ctx, ID};
use render::{RenderOptions, Renderable, EXTRA_SHAPE_POINT_RADIUS, EXTRA_SHAPE_THICKNESS};
use sim::Sim;
use std::collections::BTreeMap;

#[derive(Debug)]
enum Shape {
    Polygon(Polygon),
    Circle(Circle),
}

#[derive(Debug)]
pub struct DrawExtraShape {
    pub id: ExtraShapeID,
    shape: Shape,
    attributes: BTreeMap<String, String>,
}

impl DrawExtraShape {
    pub fn new(s: ExtraShape) -> DrawExtraShape {
        DrawExtraShape {
            id: s.id,
            shape: match s.geom {
                ExtraShapeGeom::Point(pt) => {
                    Shape::Circle(Circle::new(pt, EXTRA_SHAPE_POINT_RADIUS))
                }
                ExtraShapeGeom::Points(pl) => {
                    let width = get_sidewalk_width(&s.attributes)
                        .unwrap_or(EXTRA_SHAPE_THICKNESS * si::M)
                        .value_unsafe;
                    Shape::Polygon(pl.make_polygons(width).unwrap())
                }
            },
            attributes: s.attributes,
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

    fn tooltip_lines(&self, _map: &Map, _sim: &Sim) -> Vec<String> {
        let mut lines = Vec::new();
        for (k, v) in &self.attributes {
            // Make interesting atributes easier to spot
            if k == "TEXT" {
                lines.push(format!("*** {} = {}", k, v));
            } else {
                lines.push(format!("{} = {}", k, v));
            }
        }
        lines
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
