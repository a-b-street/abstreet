use crate::app::{App, ShowEverything};
use crate::common::ColorLegend;
use crate::game::{msg, State, Transition, WizardState};
use crate::helpers::{nice_map_name, ID};
use abstutil::{prettyprint_usize, Timer};
use ezgui::{
    hotkey, Btn, Checkbox, Choice, Color, Composite, Drawable, EventCtx, GeomBatch, GfxCtx,
    HorizontalAlignment, Key, Line, Outcome, Text, TextExt, VerticalAlignment, Widget, Wizard,
};
use geom::Distance;
use map_model::{osm, RoadID};
use sim::DontDrawAgents;
use std::collections::{BTreeMap, HashSet};
use std::fs::File;
use std::io::Write;

pub struct ParkingMapper {
    composite: Composite,
    draw_layer: Drawable,
    show_todo: bool,
    selected: Option<(HashSet<RoadID>, Drawable)>,
    hide_layer: bool,

    data: BTreeMap<i64, Value>,
}

#[derive(PartialEq, Clone)]
pub enum Value {
    BothSides,
    NoParking,
    RightOnly,
    LeftOnly,
    Complicated,
}
impl abstutil::Cloneable for Value {}

impl ParkingMapper {
    pub fn new(
        ctx: &mut EventCtx,
        app: &mut App,
        show_todo: bool,
        data: BTreeMap<i64, Value>,
    ) -> Box<dyn State> {
        app.opts.min_zoom_for_detail = 2.0;

        let map = &app.primary.map;

        let color = if show_todo {
            Color::RED.alpha(0.5)
        } else {
            Color::BLUE.alpha(0.5)
        };
        let mut batch = GeomBatch::new();
        let mut done = HashSet::new();
        let mut todo = HashSet::new();
        for r in map.all_roads() {
            if r.osm_tags.contains_key(osm::INFERRED_PARKING)
                && !data.contains_key(&r.orig_id.osm_way_id)
            {
                todo.insert(r.orig_id.osm_way_id);
                if show_todo {
                    batch.push(color, map.get_r(r.id).get_thick_polygon(map).unwrap());
                }
            } else {
                done.insert(r.orig_id.osm_way_id);
                if !show_todo {
                    batch.push(color, map.get_r(r.id).get_thick_polygon(map).unwrap());
                }
            }
        }

        // Nicer display
        for i in map.all_intersections() {
            let is_todo = i.roads.iter().any(|id| {
                let r = map.get_r(*id);
                r.osm_tags.contains_key(osm::INFERRED_PARKING)
                    && !data.contains_key(&r.orig_id.osm_way_id)
            });
            if show_todo == is_todo {
                batch.push(color, i.polygon.clone());
            }
        }

        Box::new(ParkingMapper {
            draw_layer: ctx.upload(batch),
            show_todo,
            composite: Composite::new(
                Widget::col(vec![
                    Widget::row(vec![
                        Line("Parking mapper")
                            .small_heading()
                            .draw(ctx)
                            .margin_right(10),
                        Btn::text_fg("X")
                            .build_def(ctx, hotkey(Key::Escape))
                            .align_right(),
                    ])
                    .margin_below(5),
                    Widget::row(vec![
                        "Change map:".draw_text(ctx).margin_right(10),
                        Btn::text_fg(format!("{} ▼", nice_map_name(app.primary.map.get_name())))
                            .build(ctx, "change map", None),
                    ]),
                    format!(
                        "{} / {} ways done (you've mapped {})",
                        prettyprint_usize(done.len()),
                        prettyprint_usize(done.len() + todo.len()),
                        data.len()
                    )
                    .draw_text(ctx)
                    .margin_below(5),
                    Widget::row(vec![
                        Checkbox::text(ctx, "show ways with missing tags", None, show_todo)
                            .margin_right(15),
                        ColorLegend::row(ctx, color, if show_todo { "TODO" } else { "done" }),
                    ])
                    .margin_below(5),
                    Btn::text_fg("Generate OsmChange file")
                        .build_def(ctx, None)
                        .margin_below(30),
                    "Select a road".draw_text(ctx).named("info"),
                ])
                .padding(10)
                .bg(app.cs.panel_bg),
            )
            .aligned(HorizontalAlignment::Right, VerticalAlignment::Top)
            .build(ctx),
            selected: None,
            hide_layer: false,
            data,
        })
    }

    fn make_wizard(&self, ctx: &mut EventCtx, app: &mut App) -> Box<dyn State> {
        let show_todo = self.show_todo;
        let osm_way_id = app
            .primary
            .map
            .get_r(*self.selected.as_ref().unwrap().0.iter().next().unwrap())
            .orig_id
            .osm_way_id;
        let data = self.data.clone();

        let mut state = WizardState::new(Box::new(move |wiz, ctx, app| {
            let mut wizard = wiz.wrap(ctx);
            let (_, value) = wizard.choose("What kind of parking does this road have?", || {
                vec![
                    Choice::new("none", Value::NoParking),
                    Choice::new("both sides", Value::BothSides),
                    Choice::new("just on the green side", Value::RightOnly),
                    Choice::new("just on the blue side", Value::LeftOnly),
                    Choice::new(
                        "it changes at some point along the road",
                        Value::Complicated,
                    ),
                ]
            })?;
            if value == Value::Complicated {
                wizard.acknowledge("Complicated road", || {
                    vec![
                        "You'll have to split the way in ID or JOSM and apply the parking tags to \
                         each section.",
                    ]
                })?;
            }

            let mut new_data = data.clone();
            new_data.insert(osm_way_id, value);
            Some(Transition::PopThenReplace(ParkingMapper::new(
                ctx, app, show_todo, new_data,
            )))
        }));
        state.downcast_mut::<WizardState>().unwrap().custom_pop = Some(Transition::PopThenReplace(
            ParkingMapper::new(ctx, app, self.show_todo, self.data.clone()),
        ));

        let mut batch = GeomBatch::new();
        let map = &app.primary.map;
        let thickness = Distance::meters(2.0);
        for id in &self.selected.as_ref().unwrap().0 {
            let r = map.get_r(*id);
            batch.push(
                Color::GREEN,
                map.right_shift(r.center_pts.clone(), r.width_fwd(map))
                    .unwrap()
                    .make_polygons(thickness),
            );
            batch.push(
                Color::BLUE,
                map.left_shift(r.center_pts.clone(), r.width_back(map))
                    .unwrap()
                    .make_polygons(thickness),
            );
        }
        state.downcast_mut::<WizardState>().unwrap().also_draw = Some(ctx.upload(batch));
        state
    }
}

impl State for ParkingMapper {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        let map = &app.primary.map;

        ctx.canvas_movement();
        if ctx.redo_mouseover() {
            let maybe_r = match app.calculate_current_selection(
                ctx,
                &DontDrawAgents {},
                &ShowEverything::new(),
                false,
                true,
            ) {
                Some(ID::Road(r)) => Some(r),
                Some(ID::Lane(l)) => Some(map.get_l(l).parent),
                _ => None,
            };
            if let Some(id) = maybe_r {
                if self
                    .selected
                    .as_ref()
                    .map(|(ids, _)| !ids.contains(&id))
                    .unwrap_or(true)
                {
                    // Select all roads part of this way
                    let road = map.get_r(id);
                    let way = road.orig_id.osm_way_id;
                    let mut ids = HashSet::new();
                    let mut batch = GeomBatch::new();
                    for r in map.all_roads() {
                        if r.orig_id.osm_way_id == way {
                            ids.insert(r.id);
                            batch.push(Color::CYAN.alpha(0.5), r.get_thick_polygon(map).unwrap());
                        }
                    }

                    self.selected = Some((ids, ctx.upload(batch)));

                    let mut txt = Text::new();
                    txt.add(Line(format!("Click to map parking for OSM way {}", way)));
                    for (k, v) in &road.osm_tags {
                        if k.starts_with("abst:") {
                            continue;
                        }
                        if k.contains("parking") {
                            if !road.osm_tags.contains_key(osm::INFERRED_PARKING) {
                                txt.add(Line(format!("{} = {}", k, v)));
                            }
                        } else if k == "sidewalk" {
                            if !road.osm_tags.contains_key(osm::INFERRED_SIDEWALKS) {
                                txt.add(Line(format!("{} = {}", k, v)).secondary());
                            }
                        } else {
                            txt.add(Line(format!("{} = {}", k, v)).secondary());
                        }
                    }
                    self.composite
                        .replace(ctx, "info", txt.draw(ctx).named("info"));
                }
            } else {
                self.selected = None;
                self.composite
                    .replace(ctx, "info", "Select a road".draw_text(ctx).named("info"));
            }
        }
        if self.selected.is_some() && app.per_obj.left_click(ctx, "map parking") {
            self.hide_layer = true;
            return Transition::Push(self.make_wizard(ctx, app));
        }

        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "X" => {
                    app.opts.min_zoom_for_detail =
                        crate::options::Options::default().min_zoom_for_detail;
                    return Transition::Pop;
                }
                "Generate OsmChange file" => {
                    if self.data.is_empty() {
                        return Transition::Push(msg(
                            "No changes yet",
                            vec!["Map some parking first"],
                        ));
                    }
                    ctx.loading_screen("generate OsmChange file", |_, timer| {
                        generate_osmc(&self.data, timer)
                    });
                    return Transition::Push(msg(
                        "Diff generated",
                        vec!["diff.osc created. Load it in JOSM, verify, and upload!"],
                    ));
                }
                "change map" => {
                    return Transition::Push(WizardState::new(Box::new(load_map)));
                }
                _ => unreachable!(),
            },
            None => {}
        }
        if self.composite.is_checked("show ways with missing tags") != self.show_todo {
            return Transition::Replace(ParkingMapper::new(
                ctx,
                app,
                !self.show_todo,
                self.data.clone(),
            ));
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        if !self.hide_layer {
            g.redraw(&self.draw_layer);
        }
        if let Some((_, ref roads)) = self.selected {
            g.redraw(roads);
        }
        self.composite.draw(g);
    }
}

#[cfg(target_arch = "wasm32")]
fn generate_osmc(data: &BTreeMap<i64, Value>, timer: &mut Timer) {}

#[cfg(not(target_arch = "wasm32"))]
fn generate_osmc(data: &BTreeMap<i64, Value>, timer: &mut Timer) {
    let mut modified_ways = Vec::new();
    timer.start_iter("fetch latest OSM data per modified way", data.len());
    for (way, value) in data {
        timer.next();
        if value == &Value::Complicated {
            continue;
        }

        let url = format!("https://api.openstreetmap.org/api/0.6/way/{}", way);
        timer.note(format!("Fetching {}", url));
        let resp = reqwest::blocking::get(&url).unwrap().text().unwrap();
        let mut tree = xmltree::Element::parse(resp.as_bytes())
            .unwrap()
            .take_child("way")
            .unwrap();
        let mut osm_tags = BTreeMap::new();
        let mut other_children = Vec::new();
        for node in tree.children.drain(..) {
            if let Some(elem) = node.as_element() {
                if elem.name == "tag" {
                    osm_tags.insert(elem.attributes["k"].clone(), elem.attributes["v"].clone());
                    continue;
                }
            }
            other_children.push(node);
        }

        // Fill out the tags.
        osm_tags.remove(osm::PARKING_LEFT);
        osm_tags.remove(osm::PARKING_RIGHT);
        osm_tags.remove(osm::PARKING_BOTH);
        match value {
            Value::BothSides => {
                osm_tags.insert(osm::PARKING_BOTH.to_string(), "parallel".to_string());
            }
            Value::NoParking => {
                osm_tags.insert(osm::PARKING_BOTH.to_string(), "no_parking".to_string());
            }
            Value::RightOnly => {
                osm_tags.insert(osm::PARKING_RIGHT.to_string(), "parallel".to_string());
                osm_tags.insert(osm::PARKING_LEFT.to_string(), "no_parking".to_string());
            }
            Value::LeftOnly => {
                osm_tags.insert(osm::PARKING_LEFT.to_string(), "parallel".to_string());
                osm_tags.insert(osm::PARKING_RIGHT.to_string(), "no_parking".to_string());
            }
            Value::Complicated => unreachable!(),
        }

        tree.children = other_children;
        for (k, v) in osm_tags {
            let mut new_elem = xmltree::Element::new("tag");
            new_elem.attributes.insert("k".to_string(), k);
            new_elem.attributes.insert("v".to_string(), v);
            tree.children.push(xmltree::XMLNode::Element(new_elem));
        }

        tree.attributes.remove("timestamp");
        tree.attributes.remove("changeset");
        tree.attributes.remove("user");
        tree.attributes.remove("uid");
        tree.attributes.remove("visible");

        let mut bytes: Vec<u8> = Vec::new();
        tree.write(&mut bytes).unwrap();
        let out = String::from_utf8(bytes).unwrap();
        let stripped = out.trim_start_matches("<?xml version=\"1.0\" encoding=\"UTF-8\"?>");
        modified_ways.push(stripped.to_string());
    }

    let path = "diff.osc";
    let mut f = File::create(path).unwrap();
    writeln!(f, "<osmChange version=\"0.6\" generator=\"abst\"><modify>").unwrap();
    for w in modified_ways {
        writeln!(f, "  {}", w).unwrap();
    }
    writeln!(f, "</modify></osmChange>").unwrap();
    timer.note(format!("Wrote {}", path));
}

fn load_map(wiz: &mut Wizard, ctx: &mut EventCtx, app: &mut App) -> Option<Transition> {
    let (_, name) = wiz.wrap(ctx).choose("Load map", || {
        let current_map = app.primary.map.get_name();
        abstutil::list_all_objects(abstutil::path_all_maps())
            .into_iter()
            .filter(|n| n != current_map)
            .map(|n| Choice::new(nice_map_name(&n), n.clone()))
            .collect()
    })?;
    app.switch_map(ctx, abstutil::path_map(&name));
    Some(Transition::PopThenReplace(ParkingMapper::new(
        ctx,
        app,
        true,
        BTreeMap::new(),
    )))
}
