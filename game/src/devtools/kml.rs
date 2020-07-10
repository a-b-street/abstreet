use crate::app::App;
use crate::game::{State, Transition};
use aabb_quadtree::QuadTree;
use abstutil::prettyprint_usize;
use ezgui::{
    hotkey, Btn, Choice, Color, Composite, Drawable, EventCtx, GeomBatch, GfxCtx,
    HorizontalAlignment, Key, Line, Outcome, Text, TextExt, VerticalAlignment, Widget,
};
use geom::{Circle, Distance, PolyLine, Polygon, Pt2D, Ring};
use kml::ExtraShapes;
use map_model::BuildingID;
use std::collections::{BTreeMap, HashMap, HashSet};

pub struct ViewKML {
    composite: Composite,
    objects: Vec<Object>,
    draw: Drawable,

    selected: Option<usize>,
    quadtree: QuadTree<usize>,
    query: String,
    draw_query: Drawable,
}

struct Object {
    polygon: Polygon,
    attribs: BTreeMap<String, String>,

    osm_bldg: Option<BuildingID>,
}

const RADIUS: Distance = Distance::const_meters(5.0);
const THICKNESS: Distance = Distance::const_meters(2.0);

impl ViewKML {
    pub fn new(ctx: &mut EventCtx, app: &App, path: String) -> Box<dyn State> {
        ctx.loading_screen("load kml", |ctx, mut timer| {
            let raw_shapes = if path.ends_with(".kml") {
                kml::load(&path, &app.primary.map.get_gps_bounds(), &mut timer).unwrap()
            } else {
                abstutil::read_binary::<ExtraShapes>(path.clone(), &mut timer)
            };
            let bounds = app.primary.map.get_gps_bounds();
            let boundary = app.primary.map.get_boundary_polygon();
            let dataset_name = abstutil::basename(&path);
            let bldg_lookup: HashMap<String, BuildingID> = app
                .primary
                .map
                .all_buildings()
                .iter()
                .map(|b| (b.osm_way_id.to_string(), b.id))
                .collect();
            let objects: Vec<Object> = timer
                .parallelize("convert shapes", raw_shapes.shapes, |shape| {
                    if boundary.contains_pt(Pt2D::forcibly_from_gps(shape.points[0], bounds)) {
                        let pts: Vec<Pt2D> = shape
                            .points
                            .into_iter()
                            .map(|gps| Pt2D::forcibly_from_gps(gps, bounds))
                            .collect();
                        Some(make_object(
                            &bldg_lookup,
                            shape.attributes,
                            pts,
                            &dataset_name,
                        ))
                    } else {
                        None
                    }
                })
                .into_iter()
                .flatten()
                .collect();

            let mut batch = GeomBatch::new();
            let mut quadtree = QuadTree::default(app.primary.map.get_bounds().as_bbox());
            timer.start_iter("render shapes", objects.len());
            for (idx, obj) in objects.iter().enumerate() {
                timer.next();
                quadtree.insert_with_box(idx, obj.polygon.get_bounds().as_bbox());
                batch.push(Color::RED.alpha(0.8), obj.polygon.clone());
            }

            let mut choices = vec![Choice::string("None")];
            if dataset_name == "parcels" {
                choices.push(Choice::string("parcels without buildings"));
                choices.push(Choice::string(
                    "parcels without buildings and trips or parking",
                ));
                choices.push(Choice::string("parcels with multiple buildings"));
                choices.push(Choice::string("parcels with >1 households"));
                choices.push(Choice::string("parcels with parking"));
            }

            Box::new(ViewKML {
                draw: ctx.upload(batch),
                composite: Composite::new(Widget::col(vec![
                    Widget::row(vec![
                        Line("KML viewer").small_heading().draw(ctx),
                        Btn::text_fg("X")
                            .build(ctx, "close", hotkey(Key::Escape))
                            .align_right(),
                    ]),
                    format!(
                        "{}: {} objects",
                        dataset_name,
                        prettyprint_usize(objects.len())
                    )
                    .draw_text(ctx),
                    Widget::row(vec![
                        "Query:".draw_text(ctx),
                        Widget::dropdown(ctx, "query", "None".to_string(), choices),
                    ]),
                    "Query matches 0 objects".draw_text(ctx).named("matches"),
                ]))
                .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
                .build(ctx),
                objects,
                quadtree,
                selected: None,
                query: "None".to_string(),
                draw_query: ctx.upload(GeomBatch::new()),
            })
        })
    }
}

impl State for ViewKML {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
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
                "close" => {
                    return Transition::Pop;
                }
                _ => unreachable!(),
            },
            None => {}
        }

        let query: String = self.composite.dropdown_value("query");
        if query != self.query {
            let (batch, cnt) = make_query(app, &self.objects, &query);
            self.draw_query = ctx.upload(batch);
            self.query = query;
            self.composite.replace(
                ctx,
                "matches",
                format!("Query matches {} objects", cnt)
                    .draw_text(ctx)
                    .named("matches"),
            );
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        g.redraw(&self.draw);
        g.redraw(&self.draw_query);
        self.composite.draw(g);

        if let Some(idx) = self.selected {
            let obj = &self.objects[idx];

            g.draw_polygon(Color::BLUE, &obj.polygon);
            let mut txt = Text::new();
            for (k, v) in &obj.attribs {
                txt.add(Line(format!("{} = {}", k, v)));
            }
            g.draw_mouse_tooltip(txt);

            if let Some(b) = obj.osm_bldg {
                g.draw_polygon(Color::GREEN, &app.primary.map.get_b(b).polygon);
            }
        }
    }
}

fn make_object(
    bldg_lookup: &HashMap<String, BuildingID>,
    attribs: BTreeMap<String, String>,
    pts: Vec<Pt2D>,
    dataset_name: &str,
) -> Object {
    let polygon = if pts.len() == 1 {
        Circle::new(pts[0], RADIUS).to_polygon()
    } else if pts[0] == *pts.last().unwrap() {
        // TODO Toggle between these better
        //Polygon::new(&pts)
        Ring::new(pts).make_polygons(THICKNESS)
    } else {
        PolyLine::new(pts).make_polygons(THICKNESS)
    };

    let mut osm_bldg = None;
    if dataset_name == "parcels" {
        if let Some(bldg) = attribs.get("osm_bldg") {
            if let Some(id) = bldg_lookup.get(bldg) {
                osm_bldg = Some(*id);
            }
        }
    }

    Object {
        polygon,
        attribs,
        osm_bldg,
    }
}

fn make_query(app: &App, objects: &Vec<Object>, query: &str) -> (GeomBatch, usize) {
    let mut batch = GeomBatch::new();
    let mut cnt = 0;
    let color = Color::BLUE.alpha(0.8);
    match query {
        "None" => {}
        "parcels without buildings" => {
            for obj in objects {
                if obj.osm_bldg.is_none() {
                    cnt += 1;
                    batch.push(color, obj.polygon.clone());
                }
            }
        }
        "parcels without buildings and trips or parking" => {
            for obj in objects {
                if obj.osm_bldg.is_none()
                    && (obj.attribs.contains_key("households")
                        || obj.attribs.contains_key("parking"))
                {
                    cnt += 1;
                    batch.push(color, obj.polygon.clone());
                }
            }
        }
        "parcels with multiple buildings" => {
            let mut seen = HashSet::new();
            for obj in objects {
                if let Some(b) = obj.osm_bldg {
                    if seen.contains(&b) {
                        cnt += 1;
                        batch.push(color, app.primary.map.get_b(b).polygon.clone());
                    } else {
                        seen.insert(b);
                    }
                }
            }
        }
        "parcels with >1 households" => {
            for obj in objects {
                if let Some(hh) = obj.attribs.get("households") {
                    if hh != "1" {
                        cnt += 1;
                        batch.push(color, obj.polygon.clone());
                    }
                }
            }
        }
        "parcels with parking" => {
            for obj in objects {
                if obj.attribs.contains_key("parking") {
                    cnt += 1;
                    batch.push(color, obj.polygon.clone());
                }
            }
        }
        _ => unreachable!(),
    }
    (batch, cnt)
}
