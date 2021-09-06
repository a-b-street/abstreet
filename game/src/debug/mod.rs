use std::collections::HashSet;

use abstio::MapName;
use abstutil::{Tags, Timer};
use geom::{ArrowCap, Circle, Distance, PolyLine, Pt2D};
use map_gui::colors::ColorSchemeChoice;
use map_gui::load::MapLoader;
use map_gui::options::OptionsPanel;
use map_gui::render::{calculate_corners, DrawMap, DrawOptions};
use map_gui::tools::{ChooseSomething, PopupMsg, PromptInput};
use map_gui::{AppLike, ID};
use map_model::{
    osm, ControlTrafficSignal, IntersectionID, PathConstraints, Position, RoadID,
    NORMAL_LANE_THICKNESS,
};
use sim::{Sim, TripEndpoint};
use widgetry::{
    lctrl, Cached, Choice, Color, DrawBaselayer, Drawable, EventCtx, GeomBatch, GfxCtx,
    HorizontalAlignment, Key, Line, Outcome, Panel, ScreenDims, State, Text, Toggle, UpdateType,
    VerticalAlignment, Widget,
};

use crate::app::{App, ShowLayers, ShowObject, Transition};
use crate::common::{tool_panel, CommonState};
use crate::info::ContextualActions;
use crate::sandbox::GameplayMode;

pub use self::routes::PathCostDebugger;

mod blocked_by;
mod floodfill;
mod objects;
pub mod path_counter;
mod polygons;
mod routes;
mod select_roads;
pub mod shared_row;
pub mod streetmix;
mod uber_turns;

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
    pub fn new_state(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
        Box::new(DebugMode {
            panel: Panel::new_builder(Widget::col(vec![
                Widget::row(vec![
                    Line("Debug Mode").small_heading().into_widget(ctx),
                    ctx.style().btn_close_widget(ctx),
                ]),
                Text::new().into_widget(ctx).named("current info"),
                Toggle::switch(ctx, "show buildings", Key::Num1, true),
                Toggle::switch(ctx, "show intersections", Key::Num2, true),
                Toggle::switch(ctx, "show lanes", Key::Num3, true),
                Toggle::switch(ctx, "show areas", Key::Num4, true),
                Toggle::switch(ctx, "show labels", Key::Num5, false),
                Toggle::switch(ctx, "show route for all agents", lctrl(Key::R), false),
                Toggle::switch(
                    ctx,
                    "screen recording mode",
                    lctrl(Key::H),
                    app.opts.minimal_controls,
                ),
                Widget::col(vec![
                    ctx.style()
                        .btn_outline
                        .text("unhide everything")
                        .hotkey(lctrl(Key::H))
                        .build_def(ctx),
                    ctx.style()
                        .btn_outline
                        .text("screenshot everything (for leaflet)")
                        .build_def(ctx),
                    ctx.style()
                        .btn_outline
                        .text("screenshot all of the everything")
                        .build_def(ctx),
                    ctx.style()
                        .btn_outline
                        .text("search OSM metadata")
                        .hotkey(Key::Slash)
                        .build_def(ctx),
                    ctx.style()
                        .btn_outline
                        .text("clear OSM search results")
                        .hotkey(Key::Slash)
                        .build_def(ctx),
                    ctx.style()
                        .btn_outline
                        .text("save sim state")
                        .build_def(ctx),
                    ctx.style()
                        .btn_outline
                        .text("load previous sim state")
                        .build_def(ctx),
                    ctx.style()
                        .btn_outline
                        .text("load next sim state")
                        .build_def(ctx),
                    ctx.style()
                        .btn_outline
                        .text("pick a savestate to load")
                        .build_def(ctx),
                    ctx.style()
                        .btn_outline
                        .text("find bad traffic signals")
                        .build_def(ctx),
                    ctx.style()
                        .btn_outline
                        .text("find degenerate roads")
                        .build_def(ctx),
                    ctx.style()
                        .btn_outline
                        .text("find large intersections")
                        .build_def(ctx),
                    ctx.style()
                        .btn_outline
                        .text("sim internal stats")
                        .build_def(ctx),
                    ctx.style()
                        .btn_outline
                        .text("blocked-by graph")
                        .hotkey(Key::B)
                        .build_def(ctx),
                    ctx.style()
                        .btn_outline
                        .text("render to GeoJSON")
                        .hotkey(Key::G)
                        .build_def(ctx),
                    ctx.style()
                        .btn_outline
                        .text("draw banned turns")
                        .hotkey(Key::T)
                        .build_def(ctx),
                    ctx.style()
                        .btn_outline
                        .text("draw arterial crosswalks")
                        .hotkey(Key::W)
                        .build_def(ctx),
                    ctx.style()
                        .btn_outline
                        .text("export color-scheme")
                        .build_def(ctx),
                    ctx.style()
                        .btn_outline
                        .text("import color-scheme")
                        .build_def(ctx),
                    if cfg!(not(target_arch = "wasm32")) {
                        ctx.style()
                            .btn_outline
                            .text("undo all merged roads")
                            .hotkey(lctrl(Key::M))
                            .build_def(ctx)
                    } else {
                        Widget::nothing()
                    },
                ]),
                Text::from_all(vec![
                    Line("Hold "),
                    Key::LeftControl.txt(ctx),
                    Line(" to show position"),
                ])
                .into_widget(ctx),
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
            txt.add_line(format!("Hiding {} things", self.hidden.len()));
        }
        if let Some(ref results) = self.search_results {
            txt.add_line(format!(
                "Search for {} has {} results",
                results.query, results.num_matches
            ));
        }
        if let Some((n, _)) = self.all_routes {
            txt.add_line(format!("Showing {} routes", abstutil::prettyprint_usize(n)));
        }
        self.panel
            .replace(ctx, "current info", txt.into_widget(ctx));
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
                                None => Some(Transition::Push(PopupMsg::new_state(
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
                            None => Some(Transition::Push(PopupMsg::new_state(
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
                    return Transition::Push(ChooseSomething::new_state(
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
                    return Transition::Push(PromptInput::new_state(
                        ctx,
                        "Search for what?",
                        String::new(),
                        Box::new(search_osm),
                    ));
                }
                "clear OSM search results" => {
                    self.search_results = None;
                    self.reset_info(ctx);
                }
                "screenshot everything (for leaflet)" => {
                    app.change_color_scheme(ctx, ColorSchemeChoice::DayMode);
                    export_for_leaflet(ctx, app);
                    return Transition::Keep;
                }
                "screenshot all of the everything" => {
                    return Transition::Push(ScreenshotTest::new_state(
                        ctx,
                        app,
                        vec![
                            MapName::seattle("downtown"),
                            MapName::seattle("lakeslice"),
                            MapName::seattle("montlake"),
                            MapName::seattle("udistrict"),
                            MapName::new("gb", "great_kneighton", "center"),
                            MapName::new("pl", "krakow", "center"),
                            MapName::new("us", "phoenix", "tempe"),
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
                    return Transition::Push(PopupMsg::new_state(
                        ctx,
                        "Simulation internal stats",
                        app.primary.sim.describe_internal_stats(),
                    ));
                }
                "blocked-by graph" => {
                    return Transition::Push(blocked_by::Viewer::new_state(ctx, app));
                }
                "render to GeoJSON" => {
                    // TODO Loading screen doesn't actually display anything because of the rules
                    // around hiding the first few draws
                    ctx.loading_screen("render to GeoJSON", |ctx, timer| {
                        timer.start("render");
                        let batch = DrawMap::zoomed_batch(ctx, app);
                        let features = batch.into_geojson(Some(app.primary.map.get_gps_bounds()));
                        let geojson = geojson::GeoJson::from(geojson::FeatureCollection {
                            bbox: None,
                            features,
                            foreign_members: None,
                        });
                        abstio::write_json("rendered_map.json".to_string(), &geojson);
                        timer.stop("render");
                    });
                }
                "draw banned turns" => {
                    // Abuse this just to draw
                    self.search_results = Some(SearchResults {
                        query: "banned turns".to_string(),
                        num_matches: 0,
                        draw: draw_banned_turns(ctx, app),
                    });
                    self.reset_info(ctx);
                }
                "draw arterial crosswalks" => {
                    self.search_results = Some(SearchResults {
                        query: "wide crosswalks".to_string(),
                        num_matches: 0,
                        draw: draw_arterial_crosswalks(ctx, app),
                    });
                    self.reset_info(ctx);
                }
                "export color-scheme" => {
                    app.cs.export("color_scheme").unwrap();
                }
                "import color-scheme" => {
                    app.cs.import("color_scheme").unwrap();
                    ctx.loading_screen("rerendering map colors", |ctx, timer| {
                        app.primary.draw_map =
                            DrawMap::new(ctx, &app.primary.map, &app.opts, &app.cs, timer);
                    });
                }
                #[cfg(not(target_arch = "wasm32"))]
                "undo all merged roads" => {
                    if let Err(err) =
                        std::fs::rename("merge_osm_ways.json", "UNDO_merge_osm_ways.json")
                    {
                        warn!("No merged road file? {}", err);
                    }
                    return Transition::Push(reimport_map(ctx, app, None));
                }
                _ => unreachable!(),
            },
            Outcome::Changed(_) => {
                // TODO We should really recalculate current_selection when these change. Meh.
                self.layers.show_buildings = self.panel.is_checked("show buildings");
                self.layers.show_intersections = self.panel.is_checked("show intersections");
                self.layers.show_lanes = self.panel.is_checked("show lanes");
                self.layers.show_areas = self.panel.is_checked("show areas");
                self.layers.show_labels = self.panel.is_checked("show labels");
                app.opts.minimal_controls = self.panel.is_checked("screen recording mode");
                if self.panel.is_checked("show route for all agents") {
                    if self.all_routes.is_none() {
                        self.all_routes = Some(calc_all_routes(ctx, app));
                        self.reset_info(ctx);
                    }
                } else if self.all_routes.is_some() {
                    self.all_routes = None;
                    self.reset_info(ctx);
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
                "settings" => Transition::Push(OptionsPanel::new_state(ctx, app)),
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
            batch.push(color, r.get_thick_polygon());
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
    for trace in Timer::new("calculate all routes")
        .parallelize("route to geometry", agents, |id| {
            sim.trace_route(id, map)
                .map(|trace| trace.make_polygons(NORMAL_LANE_THICKNESS))
        })
        .into_iter()
        .flatten()
    {
        cnt += 1;
        batch.push(app.cs.route, trace);
    }
    (cnt, ctx.upload(batch))
}

struct Actions;
impl ContextualActions for Actions {
    fn actions(&self, app: &App, id: ID) -> Vec<(Key, String)> {
        let mut actions = vec![
            (Key::D, "debug".to_string()),
            (Key::J, "debug with JSON viewer".to_string()),
        ];
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
                actions.push((Key::C, "export roads".to_string()));
                actions.push((Key::E, "show equiv_pos".to_string()));
                if cfg!(not(target_arch = "wasm32")) {
                    actions.push((Key::M, "merge short segment".to_string()));
                }
            }
            ID::Intersection(i) => {
                actions.push((Key::H, "hide this".to_string()));
                actions.push((Key::X, "debug intersection geometry".to_string()));
                actions.push((Key::F2, "debug sidewalk corners".to_string()));
                if app.primary.map.get_i(i).roads.len() == 2 {
                    actions.push((Key::C, "collapse degenerate road?".to_string()));
                }
                if app.primary.map.get_i(i).is_border() {
                    actions.push((Key::R, "route from here".to_string()));
                }
                actions.push((Key::U, "explore uber-turns".to_string()));
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
            ID::Building(_) => {
                actions.push((Key::R, "route from here".to_string()));
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
            (id, "debug with JSON viewer") => {
                *close_info = false;
                objects::ObjectDebugger::debug_json(id, &app.primary.map, &app.primary.sim);
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
                Transition::Push(polygons::PolygonDebugger::new_state(
                    ctx,
                    "point",
                    pts.iter().map(|pt| polygons::Item::Point(*pt)).collect(),
                    Some(Pt2D::center(&pts_without_last)),
                ))
            }
            (ID::Intersection(i), "debug sidewalk corners") => {
                Transition::Push(polygons::PolygonDebugger::new_state(
                    ctx,
                    "corner",
                    calculate_corners(app.primary.map.get_i(i), &app.primary.map)
                        .into_iter()
                        .map(polygons::Item::Polygon)
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
            (ID::Intersection(i), "route from here") => Transition::Push(
                routes::RouteExplorer::new_state(ctx, app, TripEndpoint::Border(i)),
            ),
            (ID::Intersection(i), "explore uber-turns") => {
                Transition::Push(uber_turns::UberTurnPicker::new_state(ctx, app, i))
            }
            (ID::Lane(l), "debug lane geometry") => {
                Transition::Push(polygons::PolygonDebugger::new_state(
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
                Transition::Push(polygons::PolygonDebugger::new_state(
                    ctx,
                    "triangle",
                    app.primary
                        .draw_map
                        .get_l(l)
                        .polygon
                        .triangles()
                        .into_iter()
                        .map(polygons::Item::Triangle)
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
            (ID::Lane(l), "export roads") => Transition::Push(select_roads::BulkSelect::new_state(
                ctx,
                app,
                app.primary.map.get_l(l).parent,
            )),
            (ID::Lane(l), "show equiv_pos") => {
                Transition::ModifyState(Box::new(move |state, ctx, app| {
                    if let Some(pt) = ctx.canvas.get_cursor_in_map_space() {
                        let map = &app.primary.map;
                        let pl = &map.get_l(l).lane_center_pts;
                        if let Some((dist, _)) = pl.dist_along_of_point(pl.project_pt(pt)) {
                            let base_pos = Position::new(l, dist);
                            let mut batch = GeomBatch::new();
                            for (l, _, _) in map.get_parent(l).lanes_ltr() {
                                let pt = base_pos.equiv_pos(l, map).pt(map);
                                batch.push(
                                    Color::RED,
                                    Circle::new(pt, Distance::meters(1.0)).to_polygon(),
                                );
                            }
                            let mut mode = state.downcast_mut::<DebugMode>().unwrap();
                            // Just abuse this to display the results
                            mode.search_results = Some(SearchResults {
                                query: format!("equiv_pos {}", base_pos),
                                num_matches: 0,
                                draw: ctx.upload(batch),
                            });
                        }
                    }
                }))
            }
            #[cfg(not(target_arch = "wasm32"))]
            (ID::Lane(l), "merge short segment") => {
                let mut timer = Timer::throwaway();
                let mut ways: Vec<map_model::raw::OriginalRoad> =
                    abstio::maybe_read_json("merge_osm_ways.json".to_string(), &mut timer)
                        .unwrap_or_else(|_| Vec::new());
                let orig_ways = ways.clone();
                ways.push(app.primary.map.get_parent(l).orig_id);
                abstio::write_json("merge_osm_ways.json".to_string(), &ways);
                Transition::Push(reimport_map(ctx, app, Some(orig_ways)))
            }
            (ID::Area(a), "debug area geometry") => {
                let pts = &app.primary.map.get_a(a).polygon.points();
                let center = if pts[0] == *pts.last().unwrap() {
                    // TODO The center looks really wrong for Volunteer Park and others, but I
                    // think it's because they have many points along some edges.
                    Pt2D::center(&pts.iter().skip(1).cloned().collect::<Vec<_>>())
                } else {
                    Pt2D::center(pts)
                };
                Transition::Push(polygons::PolygonDebugger::new_state(
                    ctx,
                    "point",
                    pts.iter().map(|pt| polygons::Item::Point(*pt)).collect(),
                    Some(center),
                ))
            }
            (ID::Area(a), "debug area triangles") => {
                Transition::Push(polygons::PolygonDebugger::new_state(
                    ctx,
                    "triangle",
                    app.primary
                        .map
                        .get_a(a)
                        .polygon
                        .triangles()
                        .into_iter()
                        .map(polygons::Item::Triangle)
                        .collect(),
                    None,
                ))
            }
            (ID::Building(b), "route from here") => Transition::Push(
                routes::RouteExplorer::new_state(ctx, app, TripEndpoint::Bldg(b)),
            ),
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
    error!("Bad traffic signals:");
    for i in app.primary.map.all_intersections() {
        if i.is_traffic_signal() {
            let first = &ControlTrafficSignal::get_possible_policies(&app.primary.map, i.id)[0].0;
            if first == "stage per road" || first == "arbitrary assignment" {
                error!("- {}", i.id);
            }
        }
    }
}

// Consider this a second pass to debug, after map_model/src/make/collapse_intersections.rs. Rules
// developed here will make their way there.
fn find_degenerate_roads(app: &App) {
    let map = &app.primary.map;
    for i in map.all_intersections() {
        if i.roads.len() != 2 {
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
        println!();
    }
}

fn diff_tags(t1: &Tags, t2: &Tags) {
    for (k, v1, v2) in t1.diff(t2) {
        // Ignore the most common diff
        if k == osm::OSM_WAY_ID {
            continue;
        }
        println!("- {} = \"{}\" vs \"{}\"", k, v1, v2);
    }
}

fn find_large_intersections(app: &App) {
    let mut seen = HashSet::new();
    for t in app.primary.map.all_turns() {
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
}

impl ScreenshotTest {
    fn new_state(
        ctx: &mut EventCtx,
        app: &mut App,
        mut todo_maps: Vec<MapName>,
    ) -> Box<dyn State<App>> {
        // Taking screenshots messes with options and doesn't restore them after. It's expected
        // whoever's taking screenshots (just Dustin so far) will just quit after taking them.
        app.change_color_scheme(ctx, ColorSchemeChoice::DayMode);
        app.opts.min_zoom_for_detail = 0.0;
        MapLoader::new_state(
            ctx,
            app,
            todo_maps.pop().unwrap(),
            Box::new(move |_, _| {
                Transition::Replace(Box::new(ScreenshotTest {
                    todo_maps,
                    screenshot_done: false,
                }))
            }),
        )
    }
}

impl State<App> for ScreenshotTest {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        if self.screenshot_done {
            if self.todo_maps.is_empty() {
                Transition::Pop
            } else {
                Transition::Replace(ScreenshotTest::new_state(
                    ctx,
                    app,
                    self.todo_maps.drain(..).collect(),
                ))
            }
        } else {
            self.screenshot_done = true;
            let name = app.primary.map.get_name();
            ctx.request_update(UpdateType::ScreenCaptureEverything {
                dir: format!(
                    "screenshots/{}/{}/{}",
                    name.city.country, name.city.city, name.map
                ),
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
            dir: format!(
                "screenshots/{}/{}/{}/{}",
                name.city.country, name.city.city, name.map, zoom_level
            ),
            zoom,
            dims: ScreenDims::new(256.0, 256.0),
            leaflet_naming: true,
        });
    }
}

fn draw_banned_turns(ctx: &mut EventCtx, app: &App) -> Drawable {
    let mut batch = GeomBatch::new();
    let map = &app.primary.map;
    for i in map.all_intersections() {
        let mut pairs: HashSet<(RoadID, RoadID)> = HashSet::new();
        // Don't call out one-ways, so use incoming/outgoing roads, and just for cars.
        for l1 in i.get_incoming_lanes(map, PathConstraints::Car) {
            for l2 in i.get_outgoing_lanes(map, PathConstraints::Car) {
                pairs.insert((map.get_l(l1).parent, map.get_l(l2).parent));
            }
        }
        for t in &i.turns {
            let r1 = map.get_l(t.id.src).parent;
            let r2 = map.get_l(t.id.dst).parent;
            pairs.remove(&(r1, r2));
        }

        for (r1, r2) in pairs {
            if let Ok(pl) = PolyLine::new(vec![
                map.get_r(r1).center_pts.middle(),
                map.get_r(r2).center_pts.middle(),
            ]) {
                batch.push(
                    Color::RED,
                    pl.make_arrow(Distance::meters(1.0), ArrowCap::Triangle),
                );
            }
        }
    }
    ctx.upload(batch)
}

fn draw_arterial_crosswalks(ctx: &mut EventCtx, app: &App) -> Drawable {
    let mut batch = GeomBatch::new();
    let map = &app.primary.map;
    for turn in map.all_turns() {
        if turn.is_crossing_arterial_intersection(map) {
            batch.push(
                Color::RED,
                turn.geom
                    .make_arrow(Distance::meters(2.0), ArrowCap::Triangle),
            );
        }
    }
    ctx.upload(batch)
}

#[cfg(not(target_arch = "wasm32"))]
fn reimport_map(
    ctx: &mut EventCtx,
    app: &App,
    rollback: Option<Vec<map_model::raw::OriginalRoad>>,
) -> Box<dyn State<App>> {
    map_gui::tools::RunCommand::new_state(
        ctx,
        false,
        vec![
            map_gui::tools::find_exe("importer"),
            "--map".to_string(),
            app.primary.map.get_name().map.clone(),
            format!("--city={}", app.primary.map.get_name().city.to_path()),
            "--skip_ch".to_string(),
        ],
        Box::new(|ctx, app, success, _| {
            if !success {
                if let Some(ways) = rollback {
                    if let Err(err) =
                        std::fs::copy("merge_osm_ways.json", "BROKEN_merge_osm_ways.json")
                    {
                        warn!("No merged road file? {}", err);
                    }
                    abstio::write_json("merge_osm_ways.json".to_string(), &ways);
                }
            }

            Transition::Push(MapLoader::force_reload(
                ctx,
                app.primary.map.get_name().clone(),
                Box::new(|_, _| Transition::Pop),
            ))
        }),
    )
}
