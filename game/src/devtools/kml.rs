use crate::app::App;
use crate::game::{State, Transition};
use aabb_quadtree::QuadTree;
use abstutil::prettyprint_usize;
use ezgui::{
    hotkey, Btn, Color, Composite, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key,
    Line, Outcome, Text, TextExt, VerticalAlignment, Widget,
};
use geom::{Circle, Distance, PolyLine, Polygon, Pt2D, Ring};
use kml::ExtraShapes;
use std::collections::BTreeMap;

pub struct ViewKML {
    composite: Composite,
    objects: Vec<Object>,
    draw: Drawable,

    selected: Option<usize>,
    quadtree: QuadTree<usize>,
}

struct Object {
    polygon: Polygon,
    attribs: BTreeMap<String, String>,
}

const RADIUS: Distance = Distance::const_meters(5.0);
const THICKNESS: Distance = Distance::const_meters(2.0);

impl ViewKML {
    pub fn new(ctx: &mut EventCtx, app: &App, path: &str) -> Box<dyn State> {
        ctx.loading_screen("load kml", |ctx, mut timer| {
            let raw_shapes = if path.ends_with(".kml") {
                kml::load(path, &app.primary.map.get_gps_bounds(), &mut timer).unwrap()
            } else {
                abstutil::read_binary::<ExtraShapes>(path.to_string(), &mut timer)
            };
            let bounds = app.primary.map.get_gps_bounds();

            let mut batch = GeomBatch::new();
            let mut objects = Vec::new();
            let mut quadtree = QuadTree::default(app.primary.map.get_bounds().as_bbox());
            for shape in raw_shapes.shapes {
                if !bounds.contains(shape.points[0]) {
                    continue;
                }
                let pts: Vec<Pt2D> = shape
                    .points
                    .into_iter()
                    .map(|gps| Pt2D::forcibly_from_gps(gps, bounds))
                    .collect();

                let polygon = if pts.len() == 1 {
                    Circle::new(pts[0], RADIUS).to_polygon()
                } else if pts[0] == *pts.last().unwrap() {
                    // TODO Toggle between these better
                    //Polygon::new(&pts)
                    Ring::new(pts).make_polygons(THICKNESS)
                } else {
                    PolyLine::new(pts).make_polygons(THICKNESS)
                };

                quadtree.insert_with_box(objects.len(), polygon.get_bounds().as_bbox());
                batch.push(Color::RED.alpha(0.8), polygon.clone());
                objects.push(Object {
                    polygon,
                    attribs: shape.attributes,
                });
            }

            Box::new(ViewKML {
                draw: ctx.upload(batch),
                composite: Composite::new(
                    Widget::col(vec![
                        Widget::row(vec![
                            Line("KML viewer")
                                .small_heading()
                                .draw(ctx)
                                .margin_right(10),
                            Btn::text_fg("X")
                                .build_def(ctx, hotkey(Key::Escape))
                                .align_right(),
                        ]),
                        format!("{}: {} objects", path, prettyprint_usize(objects.len()))
                            .draw_text(ctx),
                    ])
                    .padding(10)
                    .bg(app.cs.panel_bg),
                )
                .aligned(HorizontalAlignment::Right, VerticalAlignment::Top)
                .build(ctx),
                objects,
                quadtree,
                selected: None,
            })
        })
    }
}

impl State for ViewKML {
    fn event(&mut self, ctx: &mut EventCtx, _: &mut App) -> Transition {
        ctx.canvas_movement();
        if ctx.redo_mouseover() {
            self.selected = None;
            if let Some(pt) = ctx.canvas.get_cursor_in_map_space() {
                for &(idx, _, _) in &self.quadtree.query(
                    Circle::new(pt, Distance::meters(3.0))
                        .get_bounds()
                        .as_bbox(),
                ) {
                    if self.objects[*idx].polygon.contains_pt(pt) {
                        self.selected = Some(*idx);
                        break;
                    }
                }
            }
        }

        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "X" => {
                    return Transition::Pop;
                }
                _ => unreachable!(),
            },
            None => {}
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        g.redraw(&self.draw);
        self.composite.draw(g);

        if let Some(idx) = self.selected {
            g.draw_polygon(Color::BLUE, &self.objects[idx].polygon);
            let mut txt = Text::new();
            for (k, v) in &self.objects[idx].attribs {
                txt.add(Line(format!("{} = {}", k, v)));
            }
            g.draw_mouse_tooltip(txt);
        }
    }
}
