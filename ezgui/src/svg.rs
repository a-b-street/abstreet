use crate::{Color, GeomBatch};
use abstutil::VecMap;
use geom::{Bounds, Polygon, Pt2D};
use lyon::geom::{CubicBezierSegment, LineSegment};
use lyon::math::Point;
use lyon::path::PathEvent;
use lyon::tessellation;
use lyon::tessellation::geometry_builder::{simple_builder, VertexBuffers};
use lyon::tessellation::{FillVertex, StrokeVertex};

const TOLERANCE: f32 = 0.01;

// Code here adapted from
// https://github.com/nical/lyon/blob/b5c87c9a22dccfab24daa1947419a70915d60914/examples/wgpu_svg/src/main.rs.

// No offset. I'm not exactly sure how the simplification in usvg works, but this doesn't support
// transforms or strokes or text, just fills. Luckily, all of the files exported from Figma so far
// work just fine.
pub fn add_svg(batch: &mut GeomBatch, filename: &str) -> Bounds {
    let mut fill_tess = tessellation::FillTessellator::new();
    let mut stroke_tess = tessellation::StrokeTessellator::new();
    let mut fill_mesh_per_color: VecMap<Color, VertexBuffers<FillVertex, u16>> = VecMap::new();
    let mut stroke_mesh_per_color: VecMap<Color, VertexBuffers<StrokeVertex, u16>> = VecMap::new();

    let svg_tree = usvg::Tree::from_file(&filename, &usvg::Options::default()).unwrap();
    for node in svg_tree.root().descendants() {
        if let usvg::NodeKind::Path(ref p) = *node.borrow() {
            // TODO Handle transforms

            if let Some(ref fill) = p.fill {
                let color = convert_color(&fill.paint, fill.opacity.value());
                let geom = fill_mesh_per_color.mut_or_insert(color, VertexBuffers::new);
                fill_tess
                    .tessellate_path(
                        convert_path(p),
                        &tessellation::FillOptions::tolerance(TOLERANCE),
                        &mut simple_builder(geom),
                    )
                    .expect(&format!("Couldn't tesellate something from {}", filename));
            }

            if let Some(ref stroke) = p.stroke {
                let (color, stroke_opts) = convert_stroke(stroke);
                let geom = stroke_mesh_per_color.mut_or_insert(color, VertexBuffers::new);
                stroke_tess
                    .tessellate_path(convert_path(p), &stroke_opts, &mut simple_builder(geom))
                    .unwrap();
            }
        }
    }

    let mut bounds = Bounds::new();
    for (color, mesh) in fill_mesh_per_color.consume() {
        let poly = Polygon::precomputed(
            mesh.vertices
                .into_iter()
                .map(|v| Pt2D::new(v.position.x as f64, v.position.y as f64))
                .collect(),
            mesh.indices.into_iter().map(|idx| idx as usize).collect(),
            None,
        );
        bounds.union(poly.get_bounds());
        batch.push(color, poly);
    }
    for (color, mesh) in stroke_mesh_per_color.consume() {
        let poly = Polygon::precomputed(
            mesh.vertices
                .into_iter()
                .map(|v| Pt2D::new(v.position.x as f64, v.position.y as f64))
                .collect(),
            mesh.indices.into_iter().map(|idx| idx as usize).collect(),
            None,
        );
        bounds.union(poly.get_bounds());
        batch.push(color, poly);
    }
    bounds
}

fn point(x: &f64, y: &f64) -> Point {
    Point::new((*x) as f32, (*y) as f32)
}

struct PathConvIter<'a> {
    iter: std::slice::Iter<'a, usvg::PathSegment>,
    prev: Point,
    first: Point,
}

impl<'l> Iterator for PathConvIter<'l> {
    type Item = PathEvent;
    fn next(&mut self) -> Option<PathEvent> {
        match self.iter.next() {
            Some(usvg::PathSegment::MoveTo { x, y }) => {
                self.prev = point(x, y);
                self.first = self.prev;
                Some(PathEvent::MoveTo(self.prev))
            }
            Some(usvg::PathSegment::LineTo { x, y }) => {
                let from = self.prev;
                self.prev = point(x, y);
                Some(PathEvent::Line(LineSegment {
                    from,
                    to: self.prev,
                }))
            }
            Some(usvg::PathSegment::CurveTo {
                x1,
                y1,
                x2,
                y2,
                x,
                y,
            }) => {
                let from = self.prev;
                self.prev = point(x, y);
                Some(PathEvent::Cubic(CubicBezierSegment {
                    from,
                    ctrl1: point(x1, y1),
                    ctrl2: point(x2, y2),
                    to: self.prev,
                }))
            }
            Some(usvg::PathSegment::ClosePath) => {
                self.prev = self.first;
                Some(PathEvent::Close(LineSegment {
                    from: self.prev,
                    to: self.first,
                }))
            }
            None => None,
        }
    }
}

fn convert_path<'a>(p: &'a usvg::Path) -> PathConvIter<'a> {
    PathConvIter {
        iter: p.data.0.iter(),
        first: Point::new(0.0, 0.0),
        prev: Point::new(0.0, 0.0),
    }
}

fn convert_stroke(s: &usvg::Stroke) -> (Color, tessellation::StrokeOptions) {
    let color = convert_color(&s.paint, s.opacity.value());
    let linecap = match s.linecap {
        usvg::LineCap::Butt => tessellation::LineCap::Butt,
        usvg::LineCap::Square => tessellation::LineCap::Square,
        usvg::LineCap::Round => tessellation::LineCap::Round,
    };
    let linejoin = match s.linejoin {
        usvg::LineJoin::Miter => tessellation::LineJoin::Miter,
        usvg::LineJoin::Bevel => tessellation::LineJoin::Bevel,
        usvg::LineJoin::Round => tessellation::LineJoin::Round,
    };

    let opt = tessellation::StrokeOptions::tolerance(TOLERANCE)
        .with_line_width(s.width.value() as f32)
        .with_line_cap(linecap)
        .with_line_join(linejoin);

    (color, opt)
}

fn convert_color(paint: &usvg::Paint, opacity: f64) -> Color {
    if let usvg::Paint::Color(c) = paint {
        Color::rgba(
            c.red as usize,
            c.green as usize,
            c.blue as usize,
            opacity as f32,
        )
    } else {
        panic!("Unsupported paint {:?}", paint);
    }
}
