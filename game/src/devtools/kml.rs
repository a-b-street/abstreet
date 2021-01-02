// TODO Rename -- this is for KML, CSV, GeoJSON

use std::collections::{BTreeMap, HashMap, HashSet};

use aabb_quadtree::QuadTree;

use abstutil::{prettyprint_usize, Parallelism, Timer};
use geom::{Circle, Distance, PolyLine, Polygon, Pt2D, Ring};
use kml::{ExtraShape, ExtraShapes};
use map_gui::colors::ColorScheme;
use map_gui::tools::{ChooseSomething, PopupMsg};
use map_model::BuildingID;
use widgetry::{
    lctrl, Btn, Choice, Color, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key,
    Line, Outcome, Panel, State, Text, TextExt, VerticalAlignment, Widget,
};

use crate::app::{App, Transition};

pub struct ViewKML {
    panel: Panel,
    objects: Vec<Object>,
    draw: Drawable,

    selected: Option<usize>,
    quadtree: QuadTree<usize>,
    draw_query: Drawable,
}

struct Object {
    polygon: Polygon,
    color: Color,
    attribs: BTreeMap<String, String>,

    osm_bldg: Option<BuildingID>,
}

const RADIUS: Distance = Distance::const_meters(5.0);
const THICKNESS: Distance = Distance::const_meters(2.0);

impl ViewKML {
    pub fn new(ctx: &mut EventCtx, app: &App, path: Option<String>) -> Box<dyn State<App>> {
        ctx.loading_screen("load kml", |ctx, mut timer| {
            // Enable to write a smaller .bin only with the shapes matching the bounds.
            let dump_clipped_shapes = false;
            let (dataset_name, objects) = load_objects(app, path, dump_clipped_shapes, &mut timer);

            let mut batch = GeomBatch::new();
            let mut quadtree = QuadTree::default(app.primary.map.get_bounds().as_bbox());
            timer.start_iter("render shapes", objects.len());
            for (idx, obj) in objects.iter().enumerate() {
                timer.next();
                quadtree.insert_with_box(idx, obj.polygon.get_bounds().as_bbox());
                batch.push(obj.color, obj.polygon.clone());
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
                panel: Panel::new(Widget::col(vec![
                    Widget::row(vec![
                        Line("KML viewer").small_heading().draw(ctx),
                        Btn::close(ctx),
                    ]),
                    format!(
                        "{}: {} objects",
                        dataset_name,
                        prettyprint_usize(objects.len())
                    )
                    .draw_text(ctx),
                    Btn::text_fg("load KML file").build_def(ctx, lctrl(Key::L)),
                    Widget::row(vec![
                        "Query:".draw_text(ctx),
                        Widget::dropdown(ctx, "query", "None".to_string(), choices),
                    ]),
                    Widget::row(vec![
                        "Key=value filter:".draw_text(ctx),
                        Widget::text_entry(ctx, String::new(), false).named("filter"),
                    ]),
                    "Query matches 0 objects".draw_text(ctx).named("matches"),
                ]))
                .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
                .build(ctx),
                objects,
                quadtree,
                selected: None,
                draw_query: Drawable::empty(ctx),
            })
        })
    }
}

impl State<App> for ViewKML {
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
        if let Some(idx) = self.selected {
            if ctx.normal_left_click() {
                self.selected = None;
                return Transition::Push(PopupMsg::new(
                    ctx,
                    "Parcel",
                    self.objects[idx]
                        .attribs
                        .iter()
                        .map(|(k, v)| format!("{} = {}", k, v))
                        .collect(),
                ));
            }
        }

        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                "load KML file" => {
                    return Transition::Push(ChooseSomething::new(
                        ctx,
                        "Load file",
                        Choice::strings(
                            abstio::list_dir(abstio::path(format!(
                                "input/{}/",
                                app.primary.map.get_city_name()
                            )))
                            .into_iter()
                            .filter(|x| {
                                (x.ends_with(".bin") || x.ends_with(".kml") || x.ends_with(".csv"))
                                    && !x.ends_with("popdat.bin")
                            })
                            .collect(),
                        ),
                        Box::new(|path, ctx, app| {
                            Transition::Multi(vec![
                                Transition::Pop,
                                Transition::Replace(ViewKML::new(ctx, app, Some(path))),
                            ])
                        }),
                    ));
                }
                _ => unreachable!(),
            },
            Outcome::Changed => {
                let mut query: String = self.panel.dropdown_value("query");
                let filter = self.panel.text_box("filter");
                if query == "None" && !filter.is_empty() {
                    query = filter;
                }
                let (batch, cnt) = make_query(app, &self.objects, &query);
                self.draw_query = ctx.upload(batch);
                self.panel.replace(
                    ctx,
                    "matches",
                    format!("Query matches {} objects", cnt).draw_text(ctx),
                );
            }
            _ => {}
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        g.redraw(&self.draw);
        g.redraw(&self.draw_query);
        self.panel.draw(g);

        if let Some(idx) = self.selected {
            let obj = &self.objects[idx];

            g.draw_polygon(Color::BLUE, obj.polygon.clone());
            let mut txt = Text::new();
            for (k, v) in &obj.attribs {
                txt.add(Line(format!("{} = {}", k, v)));
            }
            g.draw_mouse_tooltip(txt);

            if let Some(b) = obj.osm_bldg {
                g.draw_polygon(Color::GREEN, app.primary.map.get_b(b).polygon.clone());
            }
        }
    }
}

/// Loads and clips objects to the current map. Also returns the dataset name.
fn load_objects(
    app: &App,
    path: Option<String>,
    dump_clipped_shapes: bool,
    timer: &mut Timer,
) -> (String, Vec<Object>) {
    let map = &app.primary.map;
    let bounds = map.get_gps_bounds();

    let raw_shapes = if let Some(ref path) = path {
        if path.ends_with(".kml") {
            let shapes = kml::load(&path, bounds, true, timer).unwrap();
            // Assuming this is some huge file, conveniently convert the extract to .bin.
            // The new file will show up as untracked in git, so it'll be obvious this
            // happened.
            abstio::write_binary(path.replace(".kml", ".bin"), &shapes);
            shapes
        } else if path.ends_with(".csv") {
            let shapes = ExtraShapes::load_csv(&path, bounds, timer).unwrap();
            // Assuming this is some huge file, conveniently convert the extract to .bin.
            // The new file will show up as untracked in git, so it'll be obvious this
            // happened.
            abstio::write_binary(path.replace(".csv", ".bin"), &shapes);
            shapes
        } else {
            abstio::read_binary::<ExtraShapes>(path.to_string(), timer)
        }
    } else {
        ExtraShapes { shapes: Vec::new() }
    };
    let boundary = map.get_boundary_polygon();
    let dataset_name = path
        .as_ref()
        .map(abstutil::basename)
        .unwrap_or("no file".to_string());
    let bldg_lookup: HashMap<String, BuildingID> = map
        .all_buildings()
        .iter()
        .map(|b| (b.orig_id.inner().to_string(), b.id))
        .collect();
    let cs = &app.cs;

    let pairs: Vec<(Object, ExtraShape)> = timer
        .parallelize(
            "convert shapes",
            Parallelism::Fastest,
            raw_shapes.shapes.into_iter().enumerate().collect(),
            |(idx, shape)| {
                let pts = bounds.convert(&shape.points);
                if pts.iter().any(|pt| boundary.contains_pt(*pt)) {
                    Some((
                        make_object(
                            cs,
                            &bldg_lookup,
                            shape.attributes.clone(),
                            pts,
                            &dataset_name,
                            idx,
                        ),
                        shape,
                    ))
                } else {
                    None
                }
            },
        )
        .into_iter()
        .flatten()
        .collect();
    let mut objects = Vec::new();
    let mut clipped_shapes = Vec::new();
    for (obj, shape) in pairs {
        objects.push(obj);
        clipped_shapes.push(shape);
    }
    if path.is_some() && dump_clipped_shapes {
        abstio::write_binary(
            format!("{}_clipped_for_{}.bin", dataset_name, map.get_name().map),
            &clipped_shapes,
        );
    }

    (dataset_name, objects)
}

fn make_object(
    cs: &ColorScheme,
    bldg_lookup: &HashMap<String, BuildingID>,
    attribs: BTreeMap<String, String>,
    pts: Vec<Pt2D>,
    dataset_name: &str,
    obj_idx: usize,
) -> Object {
    let mut color = Color::RED.alpha(0.8);
    let polygon = if pts.len() == 1 {
        Circle::new(pts[0], RADIUS).to_polygon()
    } else if let Ok(ring) = Ring::new(pts.clone()) {
        if attribs.get("spatial_type") == Some(&"Polygon".to_string()) {
            color = cs.rotating_color_plot(obj_idx).alpha(0.8);
            ring.to_polygon()
        } else {
            ring.to_outline(THICKNESS)
        }
    } else {
        let backup = pts[0];
        match PolyLine::new(pts) {
            Ok(pl) => pl.make_polygons(THICKNESS),
            Err(err) => {
                println!(
                    "Object with attribs {:?} has messed up geometry: {}",
                    attribs, err
                );
                Circle::new(backup, RADIUS).to_polygon()
            }
        }
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
        color,
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
        x => {
            for obj in objects {
                for (k, v) in &obj.attribs {
                    if format!("{}={}", k, v).contains(x) {
                        batch.push(color, obj.polygon.clone());
                        break;
                    }
                }
            }
        }
    }
    (batch, cnt)
}
