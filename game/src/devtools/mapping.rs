use crate::app::{App, ShowEverything};
use crate::common::{CityPicker, ColorLegend};
use crate::game::{msg, State, Transition, WizardState};
use crate::helpers::{nice_map_name, open_browser, ID};
use abstutil::{prettyprint_usize, Tags, Timer};
use ezgui::{
    hotkey, Btn, Checkbox, Choice, Color, Composite, Drawable, EventCtx, GeomBatch, GfxCtx,
    HorizontalAlignment, Key, Line, Outcome, Text, TextExt, VerticalAlignment, Widget,
};
use geom::{Distance, FindClosest, PolyLine, Polygon};
use map_model::{osm, RoadID};
use sim::DontDrawAgents;
use std::collections::{BTreeMap, HashSet};
use std::error::Error;
use std::fs::File;
use std::io::Write;

pub struct ParkingMapper {
    composite: Composite,
    draw_layer: Drawable,
    show: Show,
    selected: Option<(HashSet<RoadID>, Drawable)>,
    hide_layer: bool,

    data: BTreeMap<i64, Value>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Show {
    TODO,
    Done,
    DividedHighways,
    OverlappingStuff,
}

#[derive(PartialEq, Clone)]
pub enum Value {
    BothSides,
    NoStopping,
    RightOnly,
    LeftOnly,
    Complicated,
}
impl abstutil::Cloneable for Value {}

impl ParkingMapper {
    pub fn new(ctx: &mut EventCtx, app: &mut App) -> Box<dyn State> {
        app.primary.current_selection = None;
        ParkingMapper::make(ctx, app, Show::TODO, BTreeMap::new())
    }

    fn make(
        ctx: &mut EventCtx,
        app: &mut App,
        show: Show,
        data: BTreeMap<i64, Value>,
    ) -> Box<dyn State> {
        app.opts.min_zoom_for_detail = 2.0;

        let map = &app.primary.map;

        let color = match show {
            Show::TODO => Color::RED,
            Show::Done => Color::BLUE,
            Show::DividedHighways => Color::RED,
            Show::OverlappingStuff => Color::RED,
        }
        .alpha(0.5);
        let mut batch = GeomBatch::new();
        let mut done = HashSet::new();
        let mut todo = HashSet::new();
        for r in map.all_roads() {
            if r.is_light_rail() {
                continue;
            }
            if r.osm_tags.contains_key(osm::INFERRED_PARKING)
                && !data.contains_key(&r.orig_id.osm_way_id)
            {
                todo.insert(r.orig_id.osm_way_id);
                if show == Show::TODO {
                    batch.push(color, map.get_r(r.id).get_thick_polygon(map));
                }
            } else {
                done.insert(r.orig_id.osm_way_id);
                if show == Show::Done {
                    batch.push(color, map.get_r(r.id).get_thick_polygon(map));
                }
            }
        }
        if show == Show::DividedHighways {
            for r in find_divided_highways(app) {
                batch.push(color, map.get_r(r).get_thick_polygon(map));
            }
        }
        if show == Show::OverlappingStuff {
            ctx.loading_screen(
                "find buildings and parking lots overlapping roads",
                |_, mut timer| {
                    for poly in find_overlapping_stuff(app, &mut timer) {
                        batch.push(color, poly);
                    }
                },
            );
        }

        // Nicer display
        for i in map.all_intersections() {
            let is_todo = i.roads.iter().any(|id| {
                let r = map.get_r(*id);
                r.osm_tags.contains_key(osm::INFERRED_PARKING)
                    && !data.contains_key(&r.orig_id.osm_way_id)
            });
            if match (show, is_todo) {
                (Show::TODO, true) => true,
                (Show::Done, false) => true,
                _ => false,
            } {
                batch.push(color, i.polygon.clone());
            }
        }

        Box::new(ParkingMapper {
            draw_layer: ctx.upload(batch),
            show,
            composite: Composite::new(Widget::col(vec![
                Widget::row(vec![
                    Line("Parking mapper").small_heading().draw(ctx),
                    Btn::text_fg("X")
                        .build(ctx, "close", hotkey(Key::Escape))
                        .align_right(),
                ]),
                Widget::row(vec![
                    "Change map:".draw_text(ctx),
                    Btn::text_fg(format!("{} ↓", nice_map_name(app.primary.map.get_name()))).build(
                        ctx,
                        "change map",
                        None,
                    ),
                ]),
                format!(
                    "{} / {} ways done (you've mapped {})",
                    prettyprint_usize(done.len()),
                    prettyprint_usize(done.len() + todo.len()),
                    data.len()
                )
                .draw_text(ctx),
                Widget::row(vec![
                    Widget::dropdown(
                        ctx,
                        "Show",
                        show,
                        vec![
                            Choice::new("missing tags", Show::TODO),
                            Choice::new("already mapped", Show::Done),
                            Choice::new("divided highways", Show::DividedHighways).tooltip(
                                "Roads divided in OSM often have the wrong number of lanes tagged",
                            ),
                            Choice::new(
                                "buildings and parking lots overlapping roads",
                                Show::OverlappingStuff,
                            )
                            .tooltip("Roads often have the wrong number of lanes tagged"),
                        ],
                    ),
                    ColorLegend::row(
                        ctx,
                        color,
                        match show {
                            Show::TODO => "TODO",
                            Show::Done => "done",
                            Show::DividedHighways => "divided highways",
                            Show::OverlappingStuff => {
                                "buildings and parking lots overlapping roads"
                            }
                        },
                    ),
                ]),
                Checkbox::checkbox(ctx, "max 3 days parking (default in Seattle)", None, false),
                Btn::text_fg("Generate OsmChange file").build_def(ctx, None),
                "Select a road".draw_text(ctx).named("info"),
            ]))
            .aligned(HorizontalAlignment::Right, VerticalAlignment::Top)
            .build(ctx),
            selected: None,
            hide_layer: false,
            data,
        })
    }

    fn make_wizard(&self, ctx: &mut EventCtx, app: &mut App) -> Box<dyn State> {
        let show = self.show;
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
                    Choice::new("none -- no stopping or parking", Value::NoStopping),
                    Choice::new("both sides", Value::BothSides),
                    Choice::new("just on the green side", Value::RightOnly),
                    Choice::new("just on the blue side", Value::LeftOnly),
                    Choice::new(
                        "it changes at some point along the road",
                        Value::Complicated,
                    ),
                    Choice::new("loading zone on one or both sides", Value::Complicated),
                ]
            })?;
            if value == Value::Complicated {
                wizard.acknowledge("Complicated road", || {
                    vec![
                        "You'll have to manually split the way in ID or JOSM and apply the \
                         appropriate parking tags to each section.",
                    ]
                })?;
            }

            let mut new_data = data.clone();
            new_data.insert(osm_way_id, value);
            Some(Transition::PopThenReplace(ParkingMapper::make(
                ctx, app, show, new_data,
            )))
        }));
        state.downcast_mut::<WizardState>().unwrap().custom_pop = Some(Transition::PopThenReplace(
            ParkingMapper::make(ctx, app, self.show, self.data.clone()),
        ));

        let mut batch = GeomBatch::new();
        let map = &app.primary.map;
        let thickness = Distance::meters(2.0);
        for id in &self.selected.as_ref().unwrap().0 {
            let r = map.get_r(*id);
            batch.push(
                Color::GREEN,
                map.right_shift(r.center_pts.clone(), r.get_half_width(map))
                    .make_polygons(thickness),
            );
            batch.push(
                Color::BLUE,
                map.left_shift(r.center_pts.clone(), r.get_half_width(map))
                    .make_polygons(thickness),
            );
        }
        state.downcast_mut::<WizardState>().unwrap().also_draw =
            Some((ctx.upload(batch.clone()), ctx.upload(batch)));
        state
    }
}

impl State for ParkingMapper {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        let map = &app.primary.map;

        ctx.canvas_movement();
        if ctx.redo_mouseover() {
            let mut maybe_r = match app.calculate_current_selection(
                ctx,
                &DontDrawAgents {},
                &ShowEverything::new(),
                false,
                true,
                false,
            ) {
                Some(ID::Road(r)) => Some(r),
                Some(ID::Lane(l)) => Some(map.get_l(l).parent),
                _ => None,
            };
            if let Some(r) = maybe_r {
                if map.get_r(r).is_light_rail() {
                    maybe_r = None;
                }
            }
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
                            batch.push(Color::CYAN.alpha(0.5), r.get_thick_polygon(map));
                        }
                    }

                    self.selected = Some((ids, ctx.upload(batch)));

                    let mut txt = Text::new();
                    txt.add(Line(format!("Click to map parking for OSM way {}", way)));
                    txt.add_appended(vec![
                        Line("Shortcut: press "),
                        Line(Key::N.describe()).fg(ctx.style().hotkey_color),
                        Line(" to indicate no parking"),
                    ]);
                    txt.add_appended(vec![
                        Line("Press "),
                        Line(Key::S.describe()).fg(ctx.style().hotkey_color),
                        Line(" to open Bing StreetSide here"),
                    ]);
                    txt.add_appended(vec![
                        Line("Press "),
                        Line(Key::O.describe()).fg(ctx.style().hotkey_color),
                        Line(" to open OpenStreetMap for this way"),
                    ]);
                    for (k, v) in road.osm_tags.inner() {
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
        if self.selected.is_some() && ctx.input.key_pressed(Key::N) {
            let osm_way_id = app
                .primary
                .map
                .get_r(*self.selected.as_ref().unwrap().0.iter().next().unwrap())
                .orig_id
                .osm_way_id;
            let mut new_data = self.data.clone();
            new_data.insert(osm_way_id, Value::NoStopping);
            return Transition::Replace(ParkingMapper::make(ctx, app, self.show, new_data));
        }
        if self.selected.is_some() && ctx.input.key_pressed(Key::S) {
            if let Some(pt) = ctx.canvas.get_cursor_in_map_space() {
                let gps = pt.to_gps(app.primary.map.get_gps_bounds());
                open_browser(format!(
                    "https://www.bing.com/maps?cp={}~{}&style=x",
                    gps.y(),
                    gps.x()
                ));
            }
        }
        if let Some((ref roads, _)) = self.selected {
            if ctx.input.key_pressed(Key::O) {
                open_browser(format!(
                    "https://www.openstreetmap.org/way/{}",
                    app.primary
                        .map
                        .get_r(*roads.iter().next().unwrap())
                        .orig_id
                        .osm_way_id
                ));
            }
        }

        match self.composite.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
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
                    return match ctx.loading_screen("generate OsmChange file", |_, timer| {
                        generate_osmc(
                            &self.data,
                            self.composite
                                .is_checked("max 3 days parking (default in Seattle)"),
                            timer,
                        )
                    }) {
                        Ok(()) => Transition::Push(msg(
                            "Diff generated",
                            vec!["diff.osc created. Load it in JOSM, verify, and upload!"],
                        )),
                        Err(err) => Transition::Push(msg("Error", vec![format!("{}", err)])),
                    };
                }
                "change map" => {
                    return Transition::Push(CityPicker::new(
                        ctx,
                        app,
                        Box::new(|ctx, app| {
                            Transition::PopThenReplace(ParkingMapper::make(
                                ctx,
                                app,
                                Show::TODO,
                                BTreeMap::new(),
                            ))
                        }),
                    ));
                }
                _ => unreachable!(),
            },
            Outcome::Changed => {
                return Transition::Replace(ParkingMapper::make(
                    ctx,
                    app,
                    self.composite.dropdown_value("Show"),
                    self.data.clone(),
                ));
            }
            _ => {}
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
fn generate_osmc(_: &BTreeMap<i64, Value>, _: bool, _: &mut Timer) -> Result<(), Box<dyn Error>> {
    Err("Woops, mapping mode isn't supported on the web yet"
        .to_string()
        .into())
}

#[cfg(not(target_arch = "wasm32"))]
fn generate_osmc(
    data: &BTreeMap<i64, Value>,
    in_seattle: bool,
    timer: &mut Timer,
) -> Result<(), Box<dyn Error>> {
    let mut modified_ways = Vec::new();
    timer.start_iter("fetch latest OSM data per modified way", data.len());
    for (way, value) in data {
        timer.next();
        if value == &Value::Complicated {
            continue;
        }

        let url = format!("https://api.openstreetmap.org/api/0.6/way/{}", way);
        timer.note(format!("Fetching {}", url));
        let resp = reqwest::blocking::get(&url)?.text()?;
        let mut tree = xmltree::Element::parse(resp.as_bytes())?
            .take_child("way")
            .unwrap();
        let mut osm_tags = Tags::new(BTreeMap::new());
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
                osm_tags.insert(osm::PARKING_BOTH, "parallel");
                if in_seattle {
                    osm_tags.insert("parking:condition:both:maxstay", "3 days");
                }
            }
            Value::NoStopping => {
                osm_tags.insert(osm::PARKING_BOTH, "no_stopping");
            }
            Value::RightOnly => {
                osm_tags.insert(osm::PARKING_RIGHT, "parallel");
                osm_tags.insert(osm::PARKING_LEFT, "no_stopping");
                if in_seattle {
                    osm_tags.insert("parking:condition:right:maxstay", "3 days");
                }
            }
            Value::LeftOnly => {
                osm_tags.insert(osm::PARKING_LEFT, "parallel");
                osm_tags.insert(osm::PARKING_RIGHT, "no_stopping");
                if in_seattle {
                    osm_tags.insert("parking:condition:left:maxstay", "3 days");
                }
            }
            Value::Complicated => unreachable!(),
        }

        tree.children = other_children;
        for (k, v) in osm_tags.inner() {
            let mut new_elem = xmltree::Element::new("tag");
            new_elem.attributes.insert("k".to_string(), k.to_string());
            new_elem.attributes.insert("v".to_string(), v.to_string());
            tree.children.push(xmltree::XMLNode::Element(new_elem));
        }

        tree.attributes.remove("timestamp");
        tree.attributes.remove("changeset");
        tree.attributes.remove("user");
        tree.attributes.remove("uid");
        tree.attributes.remove("visible");

        let mut bytes: Vec<u8> = Vec::new();
        tree.write(&mut bytes)?;
        let out = String::from_utf8(bytes)?;
        let stripped = out.trim_start_matches("<?xml version=\"1.0\" encoding=\"UTF-8\"?>");
        modified_ways.push(stripped.to_string());
    }

    let mut f = File::create("diff.osc")?;
    writeln!(f, "<osmChange version=\"0.6\" generator=\"abst\"><modify>")?;
    for w in modified_ways {
        writeln!(f, "  {}", w)?;
    }
    writeln!(f, "</modify></osmChange>")?;
    timer.note(format!("Wrote diff.osc"));
    Ok(())
}

fn find_divided_highways(app: &App) -> HashSet<RoadID> {
    let map = &app.primary.map;
    let mut closest: FindClosest<RoadID> = FindClosest::new(map.get_bounds());
    // TODO Consider not even filtering by oneway. I keep finding mistakes where people split a
    // road, but didn't mark one side oneway!
    let mut oneways = Vec::new();
    for r in map.all_roads() {
        if r.osm_tags.contains_key("oneway") {
            closest.add(r.id, r.center_pts.points());
            oneways.push(r.id);
        }
    }

    let mut found = HashSet::new();
    for r1 in oneways {
        let r1 = map.get_r(r1);
        for dist in vec![
            Distance::ZERO,
            r1.center_pts.length() / 2.0,
            r1.center_pts.length(),
        ] {
            let (pt, angle) = r1.center_pts.must_dist_along(dist);
            for (r2, _, _) in closest.all_close_pts(pt, Distance::meters(250.0)) {
                if r1.id != r2
                    && PolyLine::must_new(vec![
                        pt.project_away(Distance::meters(100.0), angle.rotate_degs(90.0)),
                        pt.project_away(Distance::meters(100.0), angle.rotate_degs(-90.0)),
                    ])
                    .intersection(&map.get_r(r2).center_pts)
                    .is_some()
                    && r1.get_name() == map.get_r(r2).get_name()
                {
                    found.insert(r1.id);
                    found.insert(r2);
                }
            }
        }
    }
    found
}

// TODO Lots of false positives here... why?
fn find_overlapping_stuff(app: &App, timer: &mut Timer) -> Vec<Polygon> {
    let map = &app.primary.map;
    let mut closest: FindClosest<RoadID> = FindClosest::new(map.get_bounds());
    for r in map.all_roads() {
        if r.osm_tags.contains_key("tunnel") {
            continue;
        }
        closest.add(r.id, r.center_pts.points());
    }

    let mut polygons = Vec::new();

    timer.start_iter("check buildings", map.all_buildings().len());
    for b in map.all_buildings() {
        timer.next();
        for (r, _, _) in closest.all_close_pts(b.label_center, Distance::meters(500.0)) {
            if !b
                .polygon
                .intersection(&map.get_r(r).get_thick_polygon(map))
                .is_empty()
            {
                polygons.push(b.polygon.clone());
            }
        }
    }

    timer.start_iter("check parking lots", map.all_parking_lots().len());
    for pl in map.all_parking_lots() {
        timer.next();
        for (r, _, _) in closest.all_close_pts(pl.polygon.center(), Distance::meters(500.0)) {
            if !pl
                .polygon
                .intersection(&map.get_r(r).get_thick_polygon(map))
                .is_empty()
            {
                polygons.push(pl.polygon.clone());
            }
        }
    }

    polygons
}
