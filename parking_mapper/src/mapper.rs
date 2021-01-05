use std::collections::{BTreeMap, HashSet};

use anyhow::Result;

use abstutil::{prettyprint_usize, Timer};
use geom::{Distance, FindClosest, PolyLine, Polygon};
use map_gui::tools::{nice_map_name, open_browser, CityPicker, ColorLegend, PopupMsg};
use map_gui::{SimpleApp, ID};
use map_model::{osm, RoadID};
use osm::WayID;
use widgetry::{
    Btn, Checkbox, Choice, Color, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key,
    Line, Menu, Outcome, Panel, State, Text, TextExt, Transition, VerticalAlignment, Widget,
};

type App = SimpleApp<()>;

pub struct ParkingMapper {
    panel: Panel,
    draw_layer: Drawable,
    show: Show,
    selected: Option<(HashSet<RoadID>, Drawable)>,

    data: BTreeMap<WayID, Value>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Show {
    TODO,
    Done,
    DividedHighways,
    UnmappedDividedHighways,
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

impl ParkingMapper {
    pub fn new(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
        ParkingMapper::make(ctx, app, Show::TODO, BTreeMap::new())
    }

    fn make(
        ctx: &mut EventCtx,
        app: &App,
        show: Show,
        data: BTreeMap<WayID, Value>,
    ) -> Box<dyn State<App>> {
        let map = &app.map;

        let color = match show {
            Show::TODO => Color::RED,
            Show::Done => Color::BLUE,
            Show::DividedHighways | Show::UnmappedDividedHighways => Color::RED,
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
        if show == Show::UnmappedDividedHighways {
            for r in find_divided_highways(app) {
                let r = map.get_r(r);
                if !r.osm_tags.is("dual_carriageway", "yes") {
                    batch.push(color, r.get_thick_polygon(map));
                }
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
            panel: Panel::new(Widget::col(vec![
                Widget::row(vec![
                    Line("Parking mapper").small_heading().draw(ctx),
                    Btn::close(ctx),
                ]),
                Widget::row(vec![
                    "Change map:".draw_text(ctx),
                    Btn::pop_up(ctx, Some(nice_map_name(map.get_name()))).build(
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
                            Choice::new("unmapped divided highways", Show::UnmappedDividedHighways),
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
                            Show::UnmappedDividedHighways => "unmapped divided highways",
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
            data,
        })
    }
}

impl State<App> for ParkingMapper {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition<App> {
        let map = &app.map;

        ctx.canvas_movement();
        if ctx.redo_mouseover() {
            let mut maybe_r = match app.mouseover_unzoomed_roads_and_intersections(ctx) {
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
                        Key::N.txt(ctx),
                        Line(" to indicate no parking"),
                    ]);
                    txt.add_appended(vec![
                        Line("Press "),
                        Key::S.txt(ctx),
                        Line(" to open Bing StreetSide here"),
                    ]);
                    txt.add_appended(vec![
                        Line("Press "),
                        Key::E.txt(ctx),
                        Line(" to edit OpenStreetMap for this way"),
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
                    self.panel.replace(ctx, "info", txt.draw(ctx));
                }
            } else if self.selected.is_some() {
                self.selected = None;
                self.panel
                    .replace(ctx, "info", "Select a road".draw_text(ctx));
            }
        }
        if self.selected.is_some() && ctx.normal_left_click() {
            return Transition::Push(ChangeWay::new(
                ctx,
                app,
                &self.selected.as_ref().unwrap().0,
                self.show,
                self.data.clone(),
            ));
        }
        if self.selected.is_some() && ctx.input.pressed(Key::N) {
            let osm_way_id = map
                .get_r(*self.selected.as_ref().unwrap().0.iter().next().unwrap())
                .orig_id
                .osm_way_id;
            let mut new_data = self.data.clone();
            new_data.insert(osm_way_id, Value::NoStopping);
            return Transition::Replace(ParkingMapper::make(ctx, app, self.show, new_data));
        }
        if self.selected.is_some() && ctx.input.pressed(Key::S) {
            if let Some(pt) = ctx.canvas.get_cursor_in_map_space() {
                let gps = pt.to_gps(map.get_gps_bounds());
                open_browser(format!(
                    "https://www.bing.com/maps?cp={}~{}&style=x",
                    gps.y(),
                    gps.x()
                ));
            }
        }
        if let Some((ref roads, _)) = self.selected {
            if ctx.input.pressed(Key::E) {
                open_browser(format!(
                    "https://www.openstreetmap.org/edit?way={}",
                    map.get_r(*roads.iter().next().unwrap())
                        .orig_id
                        .osm_way_id
                        .0
                ));
            }
        }

        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                "Generate OsmChange file" => {
                    if self.data.is_empty() {
                        return Transition::Push(PopupMsg::new(
                            ctx,
                            "No changes yet",
                            vec!["Map some parking first"],
                        ));
                    }
                    return match ctx.loading_screen("generate OsmChange file", |_, timer| {
                        generate_osmc(
                            &self.data,
                            self.panel
                                .is_checked("max 3 days parking (default in Seattle)"),
                            timer,
                        )
                    }) {
                        Ok(()) => Transition::Push(PopupMsg::new(
                            ctx,
                            "Diff generated",
                            vec!["diff.osc created. Load it in JOSM, verify, and upload!"],
                        )),
                        Err(err) => {
                            Transition::Push(PopupMsg::new(ctx, "Error", vec![format!("{}", err)]))
                        }
                    };
                }
                "change map" => {
                    return Transition::Push(CityPicker::new(
                        ctx,
                        app,
                        Box::new(|ctx, app| {
                            Transition::Multi(vec![
                                Transition::Pop,
                                Transition::Replace(ParkingMapper::make(
                                    ctx,
                                    app,
                                    Show::TODO,
                                    BTreeMap::new(),
                                )),
                            ])
                        }),
                    ));
                }
                _ => unreachable!(),
            },
            Outcome::Changed => {
                return Transition::Replace(ParkingMapper::make(
                    ctx,
                    app,
                    self.panel.dropdown_value("Show"),
                    self.data.clone(),
                ));
            }
            _ => {}
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        g.redraw(&self.draw_layer);
        if let Some((_, ref roads)) = self.selected {
            g.redraw(roads);
        }
        self.panel.draw(g);
    }
}

struct ChangeWay {
    panel: Panel,
    draw: Drawable,
    osm_way_id: WayID,
    data: BTreeMap<WayID, Value>,
    show: Show,
}

impl ChangeWay {
    fn new(
        ctx: &mut EventCtx,
        app: &App,
        selected: &HashSet<RoadID>,
        show: Show,
        data: BTreeMap<WayID, Value>,
    ) -> Box<dyn State<App>> {
        let map = &app.map;
        let osm_way_id = map
            .get_r(*selected.iter().next().unwrap())
            .orig_id
            .osm_way_id;

        let mut batch = GeomBatch::new();
        let thickness = Distance::meters(2.0);
        for id in selected {
            let r = map.get_r(*id);
            batch.push(
                Color::GREEN,
                r.center_pts
                    .must_shift_right(r.get_half_width(map))
                    .make_polygons(thickness),
            );
            batch.push(
                Color::BLUE,
                r.center_pts
                    .must_shift_left(r.get_half_width(map))
                    .make_polygons(thickness),
            );
        }

        Box::new(ChangeWay {
            panel: Panel::new(Widget::col(vec![
                Widget::row(vec![
                    Line("What kind of parking does this road have?")
                        .small_heading()
                        .draw(ctx),
                    Btn::close(ctx),
                ]),
                Menu::new(
                    ctx,
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
                    ],
                )
                .named("menu"),
            ]))
            .build(ctx),
            draw: ctx.upload(batch),
            osm_way_id,
            data,
            show,
        })
    }
}

impl State<App> for ChangeWay {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition<App> {
        ctx.canvas_movement();
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => Transition::Pop,
                _ => {
                    let value = self.panel.take_menu_choice::<Value>("menu");
                    if value == Value::Complicated {
                        Transition::Replace(PopupMsg::new(
                            ctx,
                            "Complicated road",
                            vec![
                                "You'll have to manually split the way in ID or JOSM and apply \
                                 the appropriate parking tags to each section.",
                            ],
                        ))
                    } else {
                        self.data.insert(self.osm_way_id, value);
                        Transition::Multi(vec![
                            Transition::Pop,
                            Transition::Replace(ParkingMapper::make(
                                ctx,
                                app,
                                self.show,
                                self.data.clone(),
                            )),
                        ])
                    }
                }
            },
            _ => {
                if ctx.normal_left_click() && ctx.canvas.get_cursor_in_screen_space().is_none() {
                    return Transition::Pop;
                }
                Transition::Keep
            }
        }
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        g.redraw(&self.draw);
        self.panel.draw(g);
    }
}

fn generate_osmc(data: &BTreeMap<WayID, Value>, in_seattle: bool, timer: &mut Timer) -> Result<()> {
    use std::fs::File;
    use std::io::Write;

    use abstutil::Tags;

    let mut modified_ways = Vec::new();
    timer.start_iter("fetch latest OSM data per modified way", data.len());
    for (way, value) in data {
        timer.next();
        if value == &Value::Complicated {
            continue;
        }

        let url = format!("https://api.openstreetmap.org/api/0.6/way/{}", way.0);
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
    let map = &app.map;
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
                    && r1.get_name(app.opts.language.as_ref())
                        == map.get_r(r2).get_name(app.opts.language.as_ref())
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
    let map = &app.map;
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
