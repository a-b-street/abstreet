use std::collections::HashSet;

use abstio::MapName;
use abstutil::{Parallelism, Tags, Timer};
use geom::{Distance, Pt2D};
use map_gui::load::MapLoader;
use map_gui::options::OptionsPanel;
use map_gui::render::{calculate_corners, DrawMap, DrawOptions};
use map_gui::tools::{ChooseSomething, PopupMsg, PromptInput};
use map_gui::ID;
use map_model::{osm, ControlTrafficSignal, IntersectionID, NORMAL_LANE_THICKNESS};
use sim::Sim;
use widgetry::{
    lctrl, Btn, Cached, Checkbox, Choice, Color, DrawBaselayer, Drawable, EventCtx, GeomBatch,
    GfxCtx, HorizontalAlignment, Key, Line, Outcome, Panel, ScreenDims, State, Text, UpdateType,
    VerticalAlignment, Widget,
};

use crate::app::{App, ShowLayers, ShowObject, Transition};
use crate::common::{tool_panel, CommonState};
use crate::info::ContextualActions;
use crate::sandbox::GameplayMode;

mod blocked_by;
mod floodfill;
mod objects;
pub mod path_counter;
mod polygons;
pub mod shared_row;
pub mod streetmix;

pub struct DebugMode {
    panel: Panel,
    common: CommonState,
    tool_panel: Panel,
    objects: objects::ObjectDebugger,
    hidden: HashSet<ID>,
    layers: ShowLayers,
    search_results: Option<SearchResults>,
    all_routes: Option<(usize, Drawable)>,

    highlighted_agents: Cached<IntersectionID, Drawable>,
}

impl DebugMode {
    pub fn new(ctx: &mut EventCtx) -> Box<dyn State<App>> {
        Box::new(DebugMode {
            panel: Panel::new(Widget::col(vec![
                Widget::row(vec![
                    Line("Debug Mode").small_heading().draw(ctx),
                    Btn::close(ctx),
                ]),
                Text::new().draw(ctx).named("current info"),
                Checkbox::switch(ctx, "show buildings", Key::Num1, true),
                Checkbox::switch(ctx, "show intersections", Key::Num2, true),
                Checkbox::switch(ctx, "show lanes", Key::Num3, true),
                Checkbox::switch(ctx, "show areas", Key::Num4, true),
                Checkbox::switch(ctx, "show labels", Key::Num5, false),
                Checkbox::switch(ctx, "show route for all agents", Key::R, false),
                Widget::col(vec![
                    Btn::text_fg("unhide everything").build_def(ctx, lctrl(Key::H)),
                    Btn::text_fg("screenshot everything (for leaflet)").build_def(ctx, None),
                    Btn::text_fg("screenshot all of the everything").build_def(ctx, None),
                    Btn::text_fg("search OSM metadata").build_def(ctx, Key::Slash),
                    Btn::text_fg("clear OSM search results").build_def(ctx, lctrl(Key::Slash)),
                    Btn::text_fg("save sim state").build_def(ctx, Key::O),
                    Btn::text_fg("load previous sim state").build_def(ctx, Key::Y),
                    Btn::text_fg("load next sim state").build_def(ctx, Key::U),
                    Btn::text_fg("pick a savestate to load").build_def(ctx, None),
                    Btn::text_fg("find bad traffic signals").build_def(ctx, None),
                    Btn::text_fg("find degenerate roads").build_def(ctx, None),
                    Btn::text_fg("find large intersections").build_def(ctx, None),
                    Btn::text_fg("sim internal stats").build_def(ctx, None),
                    Btn::text_fg("blocked-by graph").build_def(ctx, Key::B),
                    Btn::text_fg("render to GeoJSON").build_def(ctx, Key::G),
                ]),
                Text::from_all(vec![
                    Line("Hold "),
                    Key::LeftControl.txt(ctx),
                    Line(" to show position"),
                ])
                .draw(ctx),
            ]))
            .aligned(HorizontalAlignment::Right, VerticalAlignment::Top)
            .build(ctx),
            common: CommonState::new(),
            tool_panel: tool_panel(ctx),
            objects: objects::ObjectDebugger,
            hidden: HashSet::new(),
            layers: ShowLayers::new(),
            search_results: None,
            all_routes: None,
            highlighted_agents: Cached::new(),
        })
    }

    fn reset_info(&mut self, ctx: &mut EventCtx) {
        let mut txt = Text::new();
        if !self.hidden.is_empty() {
            txt.add(Line(format!("Hiding {} things", self.hidden.len())));
        }
        if let Some(ref results) = self.search_results {
            txt.add(Line(format!(
                "Search for {} has {} results",
                results.query, results.num_matches
            )));
        }
        if let Some((n, _)) = self.all_routes {
            txt.add(Line(format!(
                "Showing {} routes",
                abstutil::prettyprint_usize(n)
            )));
        }
        self.panel.replace(ctx, "current info", txt.draw(ctx));
    }
}

impl State<App> for DebugMode {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();

        if ctx.redo_mouseover() {
            app.primary.current_selection = app.mouseover_debug_mode(ctx, self);
        }

        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                "save sim state" => {
                    ctx.loading_screen("savestate", |_, timer| {
                        timer.start("save sim state");
                        app.primary.sim.save();
                        timer.stop("save sim state");
                    });
                }
                "load previous sim state" => {
                    if let Some(t) =
                        ctx.loading_screen("load previous savestate", |ctx, mut timer| {
                            let prev_state = app
                                .primary
                                .sim
                                .find_previous_savestate(app.primary.sim.time());
                            match prev_state
                                .clone()
                                .and_then(|path| Sim::load_savestate(path, &mut timer).ok())
                            {
                                Some(new_sim) => {
                                    app.primary.sim = new_sim;
                                    app.recalculate_current_selection(ctx);
                                    None
                                }
                                None => Some(Transition::Push(PopupMsg::new(
                                    ctx,
                                    "Error",
                                    vec![format!(
                                        "Couldn't load previous savestate {:?}",
                                        prev_state
                                    )],
                                ))),
                            }
                        })
                    {
                        return t;
                    }
                }
                "load next sim state" => {
                    if let Some(t) = ctx.loading_screen("load next savestate", |ctx, mut timer| {
                        let next_state =
                            app.primary.sim.find_next_savestate(app.primary.sim.time());
                        match next_state
                            .clone()
                            .and_then(|path| Sim::load_savestate(path, &mut timer).ok())
                        {
                            Some(new_sim) => {
                                app.primary.sim = new_sim;
                                app.recalculate_current_selection(ctx);
                                None
                            }
                            None => Some(Transition::Push(PopupMsg::new(
                                ctx,
                                "Error",
                                vec![format!("Couldn't load next savestate {:?}", next_state)],
                            ))),
                        }
                    }) {
                        return t;
                    }
                }
                "pick a savestate to load" => {
                    return Transition::Push(ChooseSomething::new(
                        ctx,
                        "Load which savestate?",
                        Choice::strings(abstio::list_all_objects(app.primary.sim.save_dir())),
                        Box::new(|ss, ctx, app| {
                            // TODO Oh no, we have to do path construction here :(
                            let ss_path = format!("{}/{}.bin", app.primary.sim.save_dir(), ss);

                            ctx.loading_screen("load savestate", |ctx, mut timer| {
                                app.primary.sim = Sim::load_savestate(ss_path, &mut timer)
                                    .expect("Can't load savestate");
                                app.recalculate_current_selection(ctx);
                            });
                            Transition::Pop
                        }),
                    ));
                }
                "unhide everything" => {
                    self.hidden.clear();
                    app.primary.current_selection = app.mouseover_debug_mode(ctx, self);
                    self.reset_info(ctx);
                }
                "search OSM metadata" => {
                    return Transition::Push(PromptInput::new(
                        ctx,
                        "Search for what?",
                        Box::new(search_osm),
                    ));
                }
                "clear OSM search results" => {
                    self.search_results = None;
                    self.reset_info(ctx);
                }
                "screenshot everything (for leaflet)" => {
                    export_for_leaflet(ctx, app);
                    return Transition::Keep;
                }
                "screenshot all of the everything" => {
                    return Transition::Push(ScreenshotTest::new(
                        ctx,
                        app,
                        vec![
                            MapName::seattle("downtown"),
                            MapName::new("krakow", "center"),
                            MapName::seattle("lakeslice"),
                            MapName::seattle("montlake"),
                            MapName::new("london", "southbank"),
                            MapName::seattle("udistrict"),
                        ],
                    ));
                }
                "find bad traffic signals" => {
                    find_bad_signals(app);
                }
                "find degenerate roads" => {
                    find_degenerate_roads(app);
                }
                "find large intersections" => {
                    find_large_intersections(app);
                }
                "sim internal stats" => {
                    return Transition::Push(PopupMsg::new(
                        ctx,
                        "Simulation internal stats",
                        app.primary.sim.describe_internal_stats(),
                    ));
                }
                "blocked-by graph" => {
                    return Transition::Push(blocked_by::Viewer::new(ctx, app));
                }
                "render to GeoJSON" => {
                    // TODO Loading screen doesn't actually display anything because of the rules
                    // around hiding the first few draws
                    ctx.loading_screen("render to GeoJSON", |ctx, timer| {
                        timer.start("render");
                        let batch = DrawMap::zoomed_batch(ctx, app);
                        let features = batch.to_geojson(Some(app.primary.map.get_gps_bounds()));
                        let geojson = geojson::GeoJson::from(geojson::FeatureCollection {
                            bbox: None,
                            features,
                            foreign_members: None,
                        });
                        abstio::write_json("rendered_map.json".to_string(), &geojson);
                        timer.stop("render");
                    });
                }
                _ => unreachable!(),
            },
            Outcome::Changed => {
                // TODO We should really recalculate current_selection when these change. Meh.
                self.layers.show_buildings = self.panel.is_checked("show buildings");
                self.layers.show_intersections = self.panel.is_checked("show intersections");
                self.layers.show_lanes = self.panel.is_checked("show lanes");
                self.layers.show_areas = self.panel.is_checked("show areas");
                self.layers.show_labels = self.panel.is_checked("show labels");
                if self.panel.is_checked("show route for all agents") {
                    if self.all_routes.is_none() {
                        self.all_routes = Some(calc_all_routes(ctx, app));
                        self.reset_info(ctx);
                    }
                } else {
                    if self.all_routes.is_some() {
                        self.all_routes = None;
                        self.reset_info(ctx);
                    }
                }
            }
            _ => {}
        }

        self.highlighted_agents.update(
            match app.primary.current_selection {
                Some(ID::Intersection(i)) => Some(i),
                _ => None,
            },
            |key| {
                let mut batch = GeomBatch::new();
                for (a, _) in app.primary.sim.get_accepted_agents(key) {
                    if let Some(obj) = app.primary.draw_map.get_obj(
                        ctx,
                        ID::from_agent(a),
                        app,
                        &mut app.primary.agents.borrow_mut(),
                    ) {
                        batch.push(Color::PURPLE, obj.get_outline(&app.primary.map));
                    } else {
                        warn!(
                            "{} is accepted at or blocked by by {:?}, but no longer exists",
                            a, key
                        );
                    }
                }
                ctx.upload(batch)
            },
        );

        if let Some(t) = self.common.event(ctx, app, &mut Actions {}) {
            return t;
        }
        match self.tool_panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "back" => Transition::Pop,
                "settings" => Transition::Push(OptionsPanel::new(ctx, app)),
                _ => unreachable!(),
            },
            _ => Transition::Keep,
        }
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::Custom
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        let mut opts = DrawOptions::new();
        opts.label_buildings = self.layers.show_labels;
        app.draw(g, opts, self);

        if let Some(ref results) = self.search_results {
            g.redraw(&results.draw);
        }
        if let Some(draw) = self.highlighted_agents.value() {
            g.redraw(draw);
        }

        self.objects.draw(g, app);
        if let Some((_, ref draw)) = self.all_routes {
            g.redraw(draw);
        }

        if !g.is_screencap() {
            self.panel.draw(g);
            self.common.draw(g, app);
            self.tool_panel.draw(g);
        }
    }
}

impl ShowObject for DebugMode {
    fn show(&self, obj: &ID) -> bool {
        if self.hidden.contains(obj) {
            return false;
        }

        match obj {
            ID::Road(_) | ID::Lane(_) => self.layers.show_lanes,
            ID::Building(_) => self.layers.show_buildings,
            ID::Intersection(_) => self.layers.show_intersections,
            ID::Area(_) => self.layers.show_areas,
            _ => true,
        }
    }

    fn layers(&self) -> &ShowLayers {
        &self.layers
    }
}

fn search_osm(filter: String, ctx: &mut EventCtx, app: &mut App) -> Transition {
    let mut num_matches = 0;
    let mut batch = GeomBatch::new();

    // TODO Case insensitive
    let map = &app.primary.map;
    let color = Color::RED.alpha(0.8);
    for r in map.all_roads() {
        if r.osm_tags
            .inner()
            .iter()
            .any(|(k, v)| format!("{} = {}", k, v).contains(&filter))
        {
            num_matches += 1;
            batch.push(color, r.get_thick_polygon(map));
        }
    }
    for a in map.all_areas() {
        if a.osm_tags
            .inner()
            .iter()
            .any(|(k, v)| format!("{} = {}", k, v).contains(&filter))
        {
            num_matches += 1;
            batch.push(color, a.polygon.clone());
        }
    }

    let results = SearchResults {
        query: filter,
        num_matches,
        draw: batch.upload(ctx),
    };

    Transition::Multi(vec![
        Transition::Pop,
        Transition::ModifyState(Box::new(|state, ctx, _| {
            let mut mode = state.downcast_mut::<DebugMode>().unwrap();
            mode.search_results = Some(results);
            mode.reset_info(ctx);
        })),
    ])
}

struct SearchResults {
    query: String,
    num_matches: usize,
    draw: Drawable,
}

fn calc_all_routes(ctx: &EventCtx, app: &mut App) -> (usize, Drawable) {
    let agents = app.primary.sim.active_agents();
    let mut batch = GeomBatch::new();
    let mut cnt = 0;
    let sim = &app.primary.sim;
    let map = &app.primary.map;
    for maybe_trace in Timer::new("calculate all routes").parallelize(
        "route to geometry",
        Parallelism::Fastest,
        agents,
        |id| {
            sim.trace_route(id, map)
                .map(|trace| trace.make_polygons(NORMAL_LANE_THICKNESS))
        },
    ) {
        if let Some(t) = maybe_trace {
            cnt += 1;
            batch.push(app.cs.route, t);
        }
    }
    (cnt, ctx.upload(batch))
}

struct Actions;
impl ContextualActions for Actions {
    fn actions(&self, app: &App, id: ID) -> Vec<(Key, String)> {
        let mut actions = vec![(Key::D, "debug".to_string())];
        match id {
            ID::Lane(l) => {
                actions.push((Key::H, "hide this".to_string()));
                if app.primary.map.get_l(l).lane_type.supports_any_movement() {
                    actions.push((Key::F, "floodfill from this lane".to_string()));
                    actions.push((Key::S, "show strongly-connected components".to_string()));
                }
                actions.push((Key::X, "debug lane geometry".to_string()));
                actions.push((Key::F2, "debug lane triangles geometry".to_string()));
                actions.push((
                    Key::B,
                    "trace the block to the left of this road".to_string(),
                ));
            }
            ID::Intersection(i) => {
                actions.push((Key::H, "hide this".to_string()));
                actions.push((Key::X, "debug intersection geometry".to_string()));
                actions.push((Key::F2, "debug sidewalk corners".to_string()));
                if app.primary.map.get_i(i).roads.len() == 2 {
                    actions.push((Key::C, "collapse degenerate road?".to_string()));
                }
            }
            ID::Car(_) => {
                actions.push((Key::Backspace, "forcibly delete this car".to_string()));
            }
            ID::Area(_) => {
                actions.push((Key::X, "debug area geometry".to_string()));
                actions.push((Key::F2, "debug area triangles".to_string()));
            }
            ID::ParkingLot(_) => {
                actions.push((Key::H, "hide this".to_string()));
            }
            ID::BusStop(_) => {
                actions.push((Key::H, "hide this".to_string()));
            }
            _ => {}
        }
        actions
    }

    fn execute(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        id: ID,
        action: String,
        close_info: &mut bool,
    ) -> Transition {
        match (id, action.as_ref()) {
            (id, "hide this") => Transition::ModifyState(Box::new(|state, ctx, app| {
                let mode = state.downcast_mut::<DebugMode>().unwrap();
                println!("Hiding {:?}", id);
                app.primary.current_selection = None;
                mode.hidden.insert(id);
                mode.reset_info(ctx);
            })),
            (id, "debug") => {
                *close_info = false;
                objects::ObjectDebugger::dump_debug(id, &app.primary.map, &app.primary.sim);
                Transition::Keep
            }
            (ID::Car(c), "forcibly delete this car") => {
                app.primary.sim.delete_car(c, &app.primary.map);
                app.primary
                    .sim
                    .tiny_step(&app.primary.map, &mut app.primary.sim_cb);
                app.primary.current_selection = None;
                Transition::Keep
            }
            (ID::Lane(l), "floodfill from this lane") => {
                Transition::Push(floodfill::Floodfiller::floodfill(ctx, app, l))
            }
            (ID::Lane(l), "show strongly-connected components") => {
                Transition::Push(floodfill::Floodfiller::scc(ctx, app, l))
            }
            (ID::Intersection(i), "debug intersection geometry") => {
                let pts = app.primary.map.get_i(i).polygon.points();
                let mut pts_without_last = pts.clone();
                pts_without_last.pop();
                Transition::Push(polygons::PolygonDebugger::new(
                    ctx,
                    "point",
                    pts.iter().map(|pt| polygons::Item::Point(*pt)).collect(),
                    Some(Pt2D::center(&pts_without_last)),
                ))
            }
            (ID::Intersection(i), "debug sidewalk corners") => {
                Transition::Push(polygons::PolygonDebugger::new(
                    ctx,
                    "corner",
                    calculate_corners(app.primary.map.get_i(i), &app.primary.map)
                        .into_iter()
                        .map(|poly| polygons::Item::Polygon(poly))
                        .collect(),
                    None,
                ))
            }
            (ID::Intersection(i), "collapse degenerate road?") => {
                let i = app.primary.map.get_i(i);
                let (r1, r2) = {
                    let mut iter = i.roads.iter();
                    (*iter.next().unwrap(), *iter.next().unwrap())
                };
                diff_tags(
                    &app.primary.map.get_r(r1).osm_tags,
                    &app.primary.map.get_r(r2).osm_tags,
                );
                Transition::Keep
            }
            (ID::Lane(l), "debug lane geometry") => {
                Transition::Push(polygons::PolygonDebugger::new(
                    ctx,
                    "point",
                    app.primary
                        .map
                        .get_l(l)
                        .lane_center_pts
                        .points()
                        .iter()
                        .map(|pt| polygons::Item::Point(*pt))
                        .collect(),
                    None,
                ))
            }
            (ID::Lane(l), "debug lane triangles geometry") => {
                Transition::Push(polygons::PolygonDebugger::new(
                    ctx,
                    "triangle",
                    app.primary
                        .draw_map
                        .get_l(l)
                        .polygon
                        .triangles()
                        .into_iter()
                        .map(|tri| polygons::Item::Triangle(tri))
                        .collect(),
                    None,
                ))
            }
            (ID::Lane(l), "trace the block to the left of this road") => {
                Transition::ModifyState(Box::new(move |state, ctx, app| {
                    let mut mode = state.downcast_mut::<DebugMode>().unwrap();
                    // Just abuse this to display the results
                    mode.search_results = app
                        .primary
                        .map
                        .get_l(l)
                        .trace_around_block(&app.primary.map)
                        .map(|(poly, _)| SearchResults {
                            query: format!("block around {}", l),
                            num_matches: 0,
                            draw: ctx.upload(GeomBatch::from(vec![(Color::RED, poly)])),
                        });
                    mode.reset_info(ctx);
                }))
            }
            (ID::Area(a), "debug area geometry") => {
                let pts = &app.primary.map.get_a(a).polygon.points();
                let center = if pts[0] == *pts.last().unwrap() {
                    // TODO The center looks really wrong for Volunteer Park and others, but I
                    // think it's because they have many points along some edges.
                    Pt2D::center(&pts.iter().skip(1).cloned().collect())
                } else {
                    Pt2D::center(pts)
                };
                Transition::Push(polygons::PolygonDebugger::new(
                    ctx,
                    "point",
                    pts.iter().map(|pt| polygons::Item::Point(*pt)).collect(),
                    Some(center),
                ))
            }
            (ID::Area(a), "debug area triangles") => {
                Transition::Push(polygons::PolygonDebugger::new(
                    ctx,
                    "triangle",
                    app.primary
                        .map
                        .get_a(a)
                        .polygon
                        .triangles()
                        .into_iter()
                        .map(|tri| polygons::Item::Triangle(tri))
                        .collect(),
                    None,
                ))
            }
            _ => unreachable!(),
        }
    }

    fn is_paused(&self) -> bool {
        true
    }

    fn gameplay_mode(&self) -> GameplayMode {
        // Hack so info panels can be opened in DebugMode
        GameplayMode::FixTrafficSignals
    }
}

fn find_bad_signals(app: &App) {
    println!("Bad traffic signals:");
    for i in app.primary.map.all_intersections() {
        if i.is_traffic_signal() {
            let first = &ControlTrafficSignal::get_possible_policies(&app.primary.map, i.id)[0].0;
            if first == "stage per road" || first == "arbitrary assignment" {
                println!("- {}", i.id);
                ControlTrafficSignal::brute_force(&app.primary.map, i.id);
            }
        }
    }
}

fn find_degenerate_roads(app: &App) {
    let map = &app.primary.map;
    for i in map.all_intersections() {
        if i.roads.len() != 2 {
            continue;
        }
        if i.turns.iter().any(|t| map.get_t(*t).between_sidewalks()) {
            continue;
        }
        let (r1, r2) = {
            let mut iter = i.roads.iter();
            (*iter.next().unwrap(), *iter.next().unwrap())
        };
        let r1 = map.get_r(r1);
        let r2 = map.get_r(r2);
        if r1.zorder != r2.zorder {
            continue;
        }
        if r1
            .lanes_ltr()
            .into_iter()
            .map(|(_, dir, lt)| (dir, lt))
            .collect::<Vec<_>>()
            != r2
                .lanes_ltr()
                .into_iter()
                .map(|(_, dir, lt)| (dir, lt))
                .collect::<Vec<_>>()
        {
            continue;
        }

        println!("Maybe merge {}", i.id);
        diff_tags(&r1.osm_tags, &r2.osm_tags);
    }
}

fn diff_tags(t1: &Tags, t2: &Tags) {
    for (k, v1) in t1.inner() {
        if k == osm::OSM_WAY_ID {
            continue;
        }
        let v2 = t2.get(k).cloned().unwrap_or_else(String::new);
        if v1 != &v2 {
            println!("- {} = \"{}\" vs \"{}\"", k, v1, v2);
        }
    }
    for (k, v2) in t2.inner() {
        if !t1.contains_key(k) {
            println!("- {} = \"\" vs \"{}\"", k, v2);
        }
    }
}

fn find_large_intersections(app: &App) {
    let mut seen = HashSet::new();
    for t in app.primary.map.all_turns().values() {
        if !seen.contains(&t.id.parent) && t.geom.length() > Distance::meters(50.0) {
            println!("{} has a long turn", t.id.parent);
            seen.insert(t.id.parent);
        }
    }
}

// Because of the slightly odd control flow needed to ask widgetry to ScreenCaptureEverything, a
// separate state is the easiest way to automatically screenshot multiple maps.
struct ScreenshotTest {
    todo_maps: Vec<MapName>,
    screenshot_done: bool,
    orig_min_zoom: f64,
}

impl ScreenshotTest {
    fn new(ctx: &mut EventCtx, app: &mut App, mut todo_maps: Vec<MapName>) -> Box<dyn State<App>> {
        let orig_min_zoom = app.opts.min_zoom_for_detail;
        app.opts.min_zoom_for_detail = 0.0;
        MapLoader::new(
            ctx,
            app,
            todo_maps.pop().unwrap(),
            Box::new(move |_, _| {
                Transition::Replace(Box::new(ScreenshotTest {
                    todo_maps,
                    screenshot_done: false,
                    orig_min_zoom,
                }))
            }),
        )
    }
}

impl State<App> for ScreenshotTest {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        if self.screenshot_done {
            if self.todo_maps.is_empty() {
                app.opts.min_zoom_for_detail = self.orig_min_zoom;
                Transition::Pop
            } else {
                Transition::Replace(ScreenshotTest::new(
                    ctx,
                    app,
                    self.todo_maps.drain(..).collect(),
                ))
            }
        } else {
            self.screenshot_done = true;
            let name = app.primary.map.get_name();
            ctx.request_update(UpdateType::ScreenCaptureEverything {
                dir: format!("screenshots/{}/{}", name.city, name.map),
                zoom: 3.0,
                dims: ctx.canvas.get_window_dims(),
                leaflet_naming: false,
            });
            // TODO Sometimes this still gets stuck and needs a mouse wiggle for input event?
            Transition::Keep
        }
    }
    fn draw(&self, _: &mut GfxCtx, _: &App) {}
}

fn export_for_leaflet(ctx: &mut EventCtx, app: &App) {
    let name = app.primary.map.get_name();
    let bounds = app.primary.map.get_bounds();
    let map_length = bounds.width().max(bounds.height());

    // At zoom level N, the entire map fits into (N + 1) * (N + 1) tiles
    for zoom_level in 0..=25 {
        let num_tiles = zoom_level + 1;
        // How do we fit the entire map_length into this many tiles?
        let zoom = 256.0 * (num_tiles as f64) / map_length;
        ctx.request_update(UpdateType::ScreenCaptureEverything {
            dir: format!("screenshots/{}/{}/{}", name.city, name.map, zoom_level),
            zoom,
            dims: ScreenDims::new(256.0, 256.0),
            leaflet_naming: true,
        });
    }
}
