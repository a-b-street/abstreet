mod floodfill;
mod objects;
mod polygons;

use crate::app::{App, ShowLayers, ShowObject};
use crate::common::{tool_panel, CommonState, ContextualActions};
use crate::game::{msg, DrawBaselayer, State, Transition, WizardState};
use crate::helpers::ID;
use crate::managed::{WrappedComposite, WrappedOutcome};
use crate::render::{calculate_corners, DrawOptions};
use abstutil::Timer;
use ezgui::{
    hotkey, lctrl, Btn, Checkbox, Color, Composite, Drawable, EventCtx, EventLoopMode, GeomBatch,
    GfxCtx, HorizontalAlignment, Key, Line, Outcome, Text, VerticalAlignment, Widget, Wizard,
};
use geom::{Duration, Pt2D};
use map_model::{IntersectionID, NORMAL_LANE_THICKNESS};
use sim::{Sim, TripID};
use std::collections::HashSet;

pub struct DebugMode {
    composite: Composite,
    common: CommonState,
    tool_panel: WrappedComposite,
    objects: objects::ObjectDebugger,
    hidden: HashSet<ID>,
    layers: ShowLayers,
    search_results: Option<SearchResults>,
    all_routes: Option<(usize, Drawable)>,

    highlighted_agents: Option<(IntersectionID, Drawable)>,
}

impl DebugMode {
    pub fn new(ctx: &mut EventCtx, app: &App) -> DebugMode {
        DebugMode {
            composite: Composite::new(
                Widget::col(vec![
                    Widget::row(vec![
                        Line("Debug Mode").small_heading().draw(ctx),
                        Btn::text_fg("X")
                            .build_def(ctx, hotkey(Key::Escape))
                            .align_right(),
                    ]),
                    Text::new().draw(ctx).named("current info"),
                    Checkbox::text(ctx, "show buildings", hotkey(Key::Num1), true),
                    Checkbox::text(ctx, "show intersections", hotkey(Key::Num2), true),
                    Checkbox::text(ctx, "show lanes", hotkey(Key::Num3), true),
                    Checkbox::text(ctx, "show areas", hotkey(Key::Num4), true),
                    Checkbox::text(ctx, "show extra shapes", hotkey(Key::Num5), true),
                    Checkbox::text(ctx, "show labels", hotkey(Key::Num6), false),
                    Checkbox::text(ctx, "show route for all agents", hotkey(Key::R), false),
                    Widget::col(
                        vec![
                            (lctrl(Key::H), "unhide everything"),
                            (None, "screenshot everything"),
                            (hotkey(Key::Slash), "search OSM metadata"),
                            (lctrl(Key::Slash), "clear OSM search results"),
                            (hotkey(Key::O), "save sim state"),
                            (hotkey(Key::Y), "load previous sim state"),
                            (hotkey(Key::U), "load next sim state"),
                            (None, "pick a savestate to load"),
                        ]
                        .into_iter()
                        .map(|(key, action)| Btn::text_fg(action).build_def(ctx, key))
                        .collect(),
                    ),
                ])
                .padding(10)
                .bg(app.cs.panel_bg),
            )
            .aligned(HorizontalAlignment::Right, VerticalAlignment::Top)
            .build(ctx),
            common: CommonState::new(),
            tool_panel: tool_panel(ctx, app),
            objects: objects::ObjectDebugger::new(),
            hidden: HashSet::new(),
            layers: ShowLayers::new(),
            search_results: None,
            all_routes: None,
            highlighted_agents: None,
        }
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
        self.composite
            .replace(ctx, "current info", txt.draw(ctx).named("current info"));
    }
}

impl State for DebugMode {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();

        if ctx.redo_mouseover() {
            app.primary.current_selection =
                app.calculate_current_selection(ctx, &app.primary.sim, self, true, false);
        }

        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "X" => {
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
                            match prev_state.clone().and_then(|path| {
                                Sim::load_savestate(path, &app.primary.map, &mut timer).ok()
                            }) {
                                Some(new_sim) => {
                                    app.primary.sim = new_sim;
                                    app.recalculate_current_selection(ctx);
                                    None
                                }
                                None => Some(Transition::Push(msg(
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
                        match next_state.clone().and_then(|path| {
                            Sim::load_savestate(path, &app.primary.map, &mut timer).ok()
                        }) {
                            Some(new_sim) => {
                                app.primary.sim = new_sim;
                                app.recalculate_current_selection(ctx);
                                None
                            }
                            None => Some(Transition::Push(msg(
                                "Error",
                                vec![format!("Couldn't load next savestate {:?}", next_state)],
                            ))),
                        }
                    }) {
                        return t;
                    }
                }
                "pick a savestate to load" => {
                    return Transition::Push(WizardState::new(Box::new(load_savestate)));
                }
                "unhide everything" => {
                    self.hidden.clear();
                    app.primary.current_selection =
                        app.calculate_current_selection(ctx, &app.primary.sim, self, true, false);
                    self.reset_info(ctx);
                }
                "search OSM metadata" => {
                    return Transition::Push(WizardState::new(Box::new(search_osm)));
                }
                "clear OSM search results" => {
                    self.search_results = None;
                    self.reset_info(ctx);
                }
                "screenshot everything" => {
                    let bounds = app.primary.map.get_bounds();
                    assert!(bounds.min_x == 0.0 && bounds.min_y == 0.0);
                    return Transition::KeepWithMode(EventLoopMode::ScreenCaptureEverything {
                        dir: abstutil::path_pending_screenshots(app.primary.map.get_name()),
                        zoom: 3.0,
                        max_x: bounds.max_x,
                        max_y: bounds.max_y,
                    });
                }
                _ => unreachable!(),
            },
            None => {}
        }
        // TODO We should really recalculate current_selection when these change. Meh.
        self.layers.show_buildings = self.composite.is_checked("show buildings");
        self.layers.show_intersections = self.composite.is_checked("show intersections");
        self.layers.show_lanes = self.composite.is_checked("show lanes");
        self.layers.show_areas = self.composite.is_checked("show areas");
        self.layers.show_extra_shapes = self.composite.is_checked("show extra shapes");
        self.layers.show_labels = self.composite.is_checked("show labels");
        if self.composite.is_checked("show route for all agents") {
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

        if let Some(ID::Intersection(id)) = app.primary.current_selection {
            if self
                .highlighted_agents
                .as_ref()
                .map(|(i, _)| id != *i)
                .unwrap_or(true)
            {
                let mut batch = GeomBatch::new();
                for a in app.primary.sim.get_accepted_agents(id) {
                    batch.push(
                        Color::PURPLE,
                        app.primary
                            .draw_map
                            .get_obj(
                                ID::from_agent(a),
                                app,
                                &mut app.primary.draw_map.agents.borrow_mut(),
                                ctx.prerender,
                            )
                            .unwrap()
                            .get_outline(&app.primary.map),
                    );
                }
                self.highlighted_agents = Some((id, ctx.upload(batch)));
            }
        } else {
            self.highlighted_agents = None;
        }

        self.objects.event(ctx);

        if let Some(t) = self.common.event(ctx, app, &mut Actions {}) {
            return t;
        }
        match self.tool_panel.event(ctx, app) {
            Some(WrappedOutcome::Transition(t)) => t,
            Some(WrappedOutcome::Clicked(x)) => match x.as_ref() {
                "back" => Transition::Pop,
                _ => unreachable!(),
            },
            None => Transition::Keep,
        }
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::Custom
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        let mut opts = DrawOptions::new();
        opts.label_buildings = self.layers.show_labels;
        opts.label_roads = self.layers.show_labels;
        app.draw(g, opts, &app.primary.sim, self);

        if let Some(ref results) = self.search_results {
            g.redraw(&results.draw);
        }
        if let Some((_, ref draw)) = self.highlighted_agents {
            g.redraw(draw);
        }

        self.objects.draw(g, app);
        if let Some((_, ref draw)) = self.all_routes {
            g.redraw(draw);
        }

        if !g.is_screencap() {
            self.composite.draw(g);
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
            ID::ExtraShape(_) => self.layers.show_extra_shapes,
            ID::Area(_) => self.layers.show_areas,
            _ => true,
        }
    }

    fn layers(&self) -> &ShowLayers {
        &self.layers
    }
}

fn search_osm(wiz: &mut Wizard, ctx: &mut EventCtx, app: &mut App) -> Option<Transition> {
    let filter = wiz.wrap(ctx).input_string("Search for what?")?;
    let mut num_matches = 0;
    let mut batch = GeomBatch::new();

    // TODO Case insensitive
    let map = &app.primary.map;
    let color = Color::RED;
    for r in map.all_roads() {
        if r.osm_tags
            .iter()
            .any(|(k, v)| format!("{} = {}", k, v).contains(&filter))
        {
            num_matches += 1;
            batch.push(color, r.get_thick_polygon(map).unwrap());
        }
    }
    for b in map.all_buildings() {
        if b.osm_tags
            .iter()
            .any(|(k, v)| format!("{} = {}", k, v).contains(&filter))
            || b.amenities
                .iter()
                .any(|(n, a)| n.contains(&filter) || a.contains(&filter))
        {
            num_matches += 1;
            batch.push(color, b.polygon.clone());
        }
    }
    for a in map.all_areas() {
        if a.osm_tags
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

    Some(Transition::PopWithData(Box::new(|state, _, ctx| {
        let mut mode = state.downcast_mut::<DebugMode>().unwrap();
        mode.search_results = Some(results);
        mode.reset_info(ctx);
    })))
}

struct SearchResults {
    query: String,
    num_matches: usize,
    draw: Drawable,
}

fn load_savestate(wiz: &mut Wizard, ctx: &mut EventCtx, app: &mut App) -> Option<Transition> {
    let ss = wiz.wrap(ctx).choose_string("Load which savestate?", || {
        abstutil::list_all_objects(app.primary.sim.save_dir())
    })?;
    // TODO Oh no, we have to do path construction here :(
    let ss_path = format!("{}/{}.bin", app.primary.sim.save_dir(), ss);

    ctx.loading_screen("load savestate", |ctx, mut timer| {
        app.primary.sim = Sim::load_savestate(ss_path, &app.primary.map, &mut timer)
            .expect("Can't load savestate");
        app.recalculate_current_selection(ctx);
    });
    Some(Transition::Pop)
}

fn calc_all_routes(ctx: &EventCtx, app: &mut App) -> (usize, Drawable) {
    let trips: Vec<TripID> = app
        .primary
        .sim
        .get_trip_positions(&app.primary.map)
        .canonical_pt_per_trip
        .keys()
        .cloned()
        .collect();
    let mut batch = GeomBatch::new();
    let mut cnt = 0;
    let sim = &app.primary.sim;
    let map = &app.primary.map;
    for maybe_trace in
        Timer::new("calculate all routes").parallelize("route to geometry", trips, |trip| {
            sim.trip_to_agent(trip)
                .ok()
                .and_then(|agent| sim.trace_route(agent, map, None))
                .map(|trace| trace.make_polygons(NORMAL_LANE_THICKNESS))
        })
    {
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
            }
            ID::Intersection(_) => {
                actions.push((Key::H, "hide this".to_string()));
                actions.push((Key::X, "debug intersection geometry".to_string()));
                actions.push((Key::F2, "debug sidewalk corners".to_string()));
            }
            ID::ExtraShape(_) => {
                actions.push((Key::H, "hide this".to_string()));
            }
            ID::Car(_) => {
                actions.push((Key::Backspace, "forcibly kill this car".to_string()));
                actions.push((Key::G, "find front of blockage".to_string()));
            }
            ID::Area(_) => {
                actions.push((Key::X, "debug area geometry".to_string()));
                actions.push((Key::F2, "debug area triangles".to_string()));
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
            (id, "hide this") => Transition::KeepWithData(Box::new(|state, app, ctx| {
                let mode = state.downcast_mut::<DebugMode>().unwrap();
                println!("Hiding {:?}", id);
                app.primary.current_selection = None;
                mode.hidden.insert(id);
                mode.reset_info(ctx);
            })),
            (id, "debug") => {
                *close_info = false;
                objects::ObjectDebugger::dump_debug(
                    id,
                    &app.primary.map,
                    &app.primary.sim,
                    &app.primary.draw_map,
                );
                Transition::Keep
            }
            (ID::Car(c), "forcibly kill this car") => {
                app.primary.sim.kill_stuck_car(c, &app.primary.map);
                app.primary
                    .sim
                    .normal_step(&app.primary.map, Duration::seconds(0.1));
                app.primary.current_selection = None;
                Transition::Keep
            }
            (ID::Car(c), "find front of blockage") => Transition::Push(msg(
                "Blockage results",
                vec![format!(
                    "{} is ultimately blocked by {}",
                    c,
                    app.primary.sim.find_blockage_front(c, &app.primary.map)
                )],
            )),
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
                    app,
                    "point",
                    pts.iter().map(|pt| polygons::Item::Point(*pt)).collect(),
                    Some(Pt2D::center(&pts_without_last)),
                ))
            }
            (ID::Intersection(i), "debug sidewalk corners") => {
                Transition::Push(polygons::PolygonDebugger::new(
                    ctx,
                    app,
                    "corner",
                    calculate_corners(
                        app.primary.map.get_i(i),
                        &app.primary.map,
                        &mut Timer::new("calculate corners"),
                    )
                    .into_iter()
                    .map(|poly| polygons::Item::Polygon(poly))
                    .collect(),
                    None,
                ))
            }
            (ID::Lane(l), "debug lane geometry") => {
                Transition::Push(polygons::PolygonDebugger::new(
                    ctx,
                    app,
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
                    app,
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
                    app,
                    "point",
                    pts.iter().map(|pt| polygons::Item::Point(*pt)).collect(),
                    Some(center),
                ))
            }
            (ID::Area(a), "debug area triangles") => {
                Transition::Push(polygons::PolygonDebugger::new(
                    ctx,
                    app,
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
}
