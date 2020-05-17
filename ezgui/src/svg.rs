use crate::{Color, FancyColor, GeomBatch, LinearGradient, Prerender};
use abstutil::VecMap;
use geom::{Bounds, Polygon, Pt2D};
use lyon::math::Point;
use lyon::path::PathEvent;
use lyon::tessellation;
use lyon::tessellation::geometry_builder::{simple_builder, VertexBuffers};

pub const HIGH_QUALITY: f32 = 0.01;
pub const LOW_QUALITY: f32 = 1.0;

// Code here adapted from
// https://github.com/nical/lyon/blob/0d0ee771180fb317b986d9cf30266722e0773e01/examples/wgpu_svg/src/main.rs

pub fn load_svg(prerender: &Prerender, filename: &str, scale_factor: f64) -> (GeomBatch, Bounds) {
    if let Some(pair) = prerender.assets.get_cached_svg(filename, scale_factor) {
        return pair;
    }

    let raw = if let Ok(raw) = abstutil::slurp_file(&filename) {
        raw
    } else {
        panic!("Can't read {}", filename);
    };
    let svg_tree = usvg::Tree::from_data(&raw, &usvg::Options::default()).unwrap();
    let mut batch = GeomBatch::new();
    match add_svg_inner(&mut batch, svg_tree, HIGH_QUALITY, scale_factor) {
        Ok(bounds) => {
            prerender.assets.cache_svg(
                filename.to_string(),
                scale_factor,
                batch.clone(),
                bounds.clone(),
            );
            (batch, bounds)
        }
        Err(err) => panic!("{}: {}", filename, err),
    }
}

// No offset. I'm not exactly sure how the simplification in usvg works, but this doesn't support
// transforms or strokes or text, just fills. Luckily, all of the files exported from Figma so far
// work just fine.
pub fn add_svg_inner(
    batch: &mut GeomBatch,
    svg_tree: usvg::Tree,
    tolerance: f32,
    scale: f64,
) -> Result<Bounds, String> {
    let mut fill_tess = tessellation::FillTessellator::new();
    let mut stroke_tess = tessellation::StrokeTessellator::new();
    // TODO This breaks on start.svg; the order there matters. color1, color2, then color1 again.
    let mut mesh_per_color: VecMap<FancyColor, VertexBuffers<_, u16>> = VecMap::new();

    for node in svg_tree.root().descendants() {
        if let usvg::NodeKind::Path(ref p) = *node.borrow() {
            // TODO Handle transforms

            if let Some(ref fill) = p.fill {
                let color = convert_color(&fill.paint, fill.opacity.value(), &svg_tree);
                let geom = mesh_per_color.mut_or_insert(color, VertexBuffers::new);
                if fill_tess
                    .tessellate(
                        convert_path(p),
                        &tessellation::FillOptions::tolerance(tolerance),
                        &mut simple_builder(geom),
                    )
                    .is_err()
                {
                    return Err(format!("Couldn't tesellate something"));
                }
            }

            if let Some(ref stroke) = p.stroke {
                let (color, stroke_opts) = convert_stroke(stroke, tolerance, &svg_tree);
                let geom = mesh_per_color.mut_or_insert(color, VertexBuffers::new);
                stroke_tess
                    .tessellate(convert_path(p), &stroke_opts, &mut simple_builder(geom))
                    .unwrap();
            }
        }
    }

    for (color, mesh) in mesh_per_color.consume() {
        batch.fancy_push(
            color,
            Polygon::precomputed(
                mesh.vertices
                    .into_iter()
                    .map(|v| Pt2D::new(scale * f64::from(v.x), scale * f64::from(v.y)))
                    .collect(),
                mesh.indices.into_iter().map(|idx| idx as usize).collect(),
            ),
        );
    }
    let size = svg_tree.svg_node().size;
    Ok(Bounds::from(&vec![
        Pt2D::new(0.0, 0.0),
        Pt2D::new(scale * size.width(), scale * size.height()),
    ]))
}

fn point(x: &f64, y: &f64) -> Point {
    Point::new((*x) as f32, (*y) as f32)
}

struct PathConvIter<'a> {
    iter: std::slice::Iter<'a, usvg::PathSegment>,
    prev: Point,
    first: Point,
    needs_end: bool,
    deferred: Option<PathEvent>,
}

impl<'l> Iterator for PathConvIter<'l> {
    type Item = PathEvent;
    fn next(&mut self) -> Option<PathEvent> {
        if self.deferred.is_some() {
            return self.deferred.take();
        }

        let next = self.iter.next();
        match next {
            Some(usvg::PathSegment::MoveTo { x, y }) => {
                if self.needs_end {
                    let last = self.prev;
                    let first = self.first;
                    self.needs_end = false;
                    self.prev = point(x, y);
                    self.deferred = Some(PathEvent::Begin { at: self.prev });
                    self.first = self.prev;
                    Some(PathEvent::End {
                        last,
                        first,
                        close: false,
                    })
                } else {
                    self.first = point(x, y);
                    Some(PathEvent::Begin { at: self.first })
                }
            }
            Some(usvg::PathSegment::LineTo { x, y }) => {
                self.needs_end = true;
                let from = self.prev;
                self.prev = point(x, y);
                Some(PathEvent::Line {
                    from,
                    to: self.prev,
                })
            }
            Some(usvg::PathSegment::CurveTo {
                x1,
                y1,
                x2,
                y2,
                x,
                y,
            }) => {
                self.needs_end = true;
                let from = self.prev;
                self.prev = point(x, y);
                Some(PathEvent::Cubic {
                    from,
                    ctrl1: point(x1, y1),
                    ctrl2: point(x2, y2),
                    to: self.prev,
                })
            }
            Some(usvg::PathSegment::ClosePath) => {
                self.needs_end = false;
                self.prev = self.first;
                Some(PathEvent::End {
                    last: self.prev,
                    first: self.first,
                    close: true,
                })
            }
            None => {
                if self.needs_end {
                    self.needs_end = false;
                    let last = self.prev;
                    let first = self.first;
                    Some(PathEvent::End {
                        last,
                        first,
                        close: false,
                    })
                } else {
                    None
                }
            }
        }
    }
}

fn convert_path<'a>(p: &'a usvg::Path) -> PathConvIter<'a> {
    PathConvIter {
        iter: p.data.0.iter(),
        first: Point::new(0.0, 0.0),
        prev: Point::new(0.0, 0.0),
        deferred: None,
        needs_end: false,
    }
}

fn convert_stroke(
    s: &usvg::Stroke,
    tolerance: f32,
    tree: &usvg::Tree,
) -> (FancyColor, tessellation::StrokeOptions) {
    let color = convert_color(&s.paint, s.opacity.value(), tree);
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

    let opt = tessellation::StrokeOptions::tolerance(tolerance)
        .with_line_width(s.width.value() as f32)
        .with_line_cap(linecap)
        .with_line_join(linejoin);

    (color, opt)
}

fn convert_color(paint: &usvg::Paint, opacity: f64, tree: &usvg::Tree) -> FancyColor {
    match paint {
        usvg::Paint::Color(c) => FancyColor::RGBA(Color::rgba(
            c.red as usize,
            c.green as usize,
            c.blue as usize,
            opacity as f32,
        )),
        usvg::Paint::Link(name) => match *tree.defs_by_id(name).unwrap().borrow() {
            usvg::NodeKind::LinearGradient(ref lg) => LinearGradient::new(lg),
            _ => panic!("Unsupported color style {}", name),
        },
    }
}
