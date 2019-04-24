use crate::objects::{DrawCtx, RenderingHints, ID};
use crate::render::{
    draw_vehicle, AgentCache, DrawPedestrian, RenderOptions, Renderable, MIN_ZOOM_FOR_DETAIL,
};
use crate::state::UIState;
use abstutil;
use ezgui::{
    Canvas, Color, EventCtx, EventLoopMode, Folder, GfxCtx, Key, ModalMenu, Prerender, Text,
    TopMenu, BOTTOM_LEFT, GUI,
};
use geom::{Bounds, Circle, Distance, Polygon};
use map_model::{BuildingID, LaneID, Traversable};
use serde_derive::{Deserialize, Serialize};
use std::collections::HashSet;

// TODO Collapse stuff!
pub struct UI {
    pub hints: RenderingHints,
    pub state: UIState,
}

impl GUI for UI {
    fn top_menu(&self, canvas: &Canvas) -> Option<TopMenu> {
        let mut folders = Vec::new();
        folders.push(Folder::new(
            "File",
            vec![
                (Some(Key::L), "show legend"),
                (Some(Key::Escape), "pause game"),
            ],
        ));
        if self.state.enable_debug_controls {
            folders.push(Folder::new(
                "Debug",
                vec![
                    (None, "screenshot everything"),
                    (Some(Key::F1), "screenshot just this"),
                    (None, "find chokepoints"),
                    (None, "validate map geometry"),
                    (Some(Key::Num1), "show/hide buildings"),
                    (Some(Key::Num2), "show/hide intersections"),
                    (Some(Key::Num3), "show/hide lanes"),
                    (Some(Key::Num5), "show/hide areas"),
                    (Some(Key::Num6), "show OSM colors"),
                    (Some(Key::Num7), "show/hide extra shapes"),
                    (Some(Key::Num9), "show/hide all turn icons"),
                    (None, "show/hide geometry debug mode"),
                ],
            ));
        }
        folders.extend(vec![
            Folder::new(
                "Edit",
                vec![
                    (Some(Key::B), "manage A/B tests"),
                    (None, "configure colors"),
                    (Some(Key::N), "manage neighborhoods"),
                    (Some(Key::E), "edit roads"),
                    (Some(Key::W), "manage scenarios"),
                ],
            ),
            Folder::new(
                "Simulation",
                vec![
                    (Some(Key::LeftBracket), "slow down sim"),
                    (Some(Key::RightBracket), "speed up sim"),
                    (Some(Key::O), "save sim state"),
                    (Some(Key::Y), "load previous sim state"),
                    (Some(Key::U), "load next sim state"),
                    (Some(Key::Space), "run/pause sim"),
                    (Some(Key::M), "run one step of sim"),
                    (Some(Key::Dot), "show/hide sim info sidepanel"),
                    (Some(Key::T), "start time traveling"),
                    (Some(Key::D), "diff all A/B trips"),
                    (Some(Key::S), "seed the sim with agents"),
                    (Some(Key::LeftAlt), "swap the primary/secondary sim"),
                    (None, "reset sim"),
                ],
            ),
            Folder::new(
                "View",
                vec![
                    (None, "show neighborhood summaries"),
                    (Some(Key::Slash), "search for something"),
                    (Some(Key::A), "show lanes with active traffic"),
                    (Some(Key::J), "warp to an object"),
                ],
            ),
        ]);
        Some(TopMenu::new(folders, canvas))
    }

    fn modal_menus(&self) -> Vec<ModalMenu> {
        vec![
            ModalMenu::new(
                "Traffic Signal Editor",
                vec![
                    (Key::Enter, "quit"),
                    (Key::D, "change cycle duration"),
                    (Key::P, "choose a preset signal"),
                    (Key::K, "move current cycle up"),
                    (Key::J, "move current cycle down"),
                    (Key::UpArrow, "select previous cycle"),
                    (Key::DownArrow, "select next cycle"),
                    (Key::Backspace, "delete current cycle"),
                    (Key::N, "add a new empty cycle"),
                    (Key::M, "add a new pedestrian scramble cycle"),
                ],
            ),
            ModalMenu::new(
                "Scenario Editor",
                vec![
                    (Key::S, "save"),
                    (Key::E, "edit"),
                    (Key::I, "instantiate"),
                    (Key::V, "visualize"),
                    (Key::Enter, "quit"),
                ],
            ),
            ModalMenu::new("Road Editor", vec![(Key::Enter, "quit")]),
            ModalMenu::new(
                "Color Picker",
                vec![(Key::Backspace, "revert"), (Key::Enter, "finalize")],
            ),
            ModalMenu::new(
                "Stop Sign Editor",
                vec![(Key::Enter, "quit"), (Key::R, "reset to default")],
            ),
            ModalMenu::new("A/B Test Editor", vec![(Key::R, "run A/B test")]),
            ModalMenu::new(
                "Neighborhood Editor",
                vec![
                    (Key::Enter, "save"),
                    (Key::Q, "quit"),
                    (Key::X, "export as an Osmosis polygon filter"),
                    (Key::P, "add a new point"),
                ],
            ),
            ModalMenu::new(
                "Time Traveler",
                vec![
                    (Key::Enter, "quit"),
                    (Key::Comma, "rewind"),
                    (Key::Dot, "forwards"),
                ],
            ),
            ModalMenu::new(
                "Simple Model",
                vec![
                    (Key::Enter, "quit"),
                    (Key::Comma, "rewind"),
                    (Key::Dot, "forwards"),
                    (Key::Space, "toggle forwards play"),
                    (Key::M, "toggle backwards play"),
                    (Key::T, "toggle tooltips"),
                    (Key::E, "exhaustively test instantiation everywhere"),
                    (Key::D, "debug"),
                ],
            ),
            ModalMenu::new(
                "Even Simpler Model",
                vec![
                    (Key::Enter, "quit"),
                    (Key::Dot, "forwards"),
                    (Key::Space, "toggle forwards play"),
                    (Key::E, "spawn tons of cars everywhere"),
                ],
            ),
            ModalMenu::new(
                "Geometry Debugger",
                vec![(Key::Enter, "quit"), (Key::N, "see next problem")],
            ),
            ModalMenu::new("Original Roads", vec![(Key::Enter, "quit")]),
            ModalMenu::new("OSM Classifier", vec![(Key::Num6, "quit")]),
            ModalMenu::new(
                "Floodfiller",
                vec![
                    (Key::Enter, "quit"),
                    (Key::Space, "step forwards"),
                    (Key::Tab, "finish floodfilling"),
                ],
            ),
            ModalMenu::new("Chokepoints Debugger", vec![(Key::Enter, "quit")]),
            ModalMenu::new("A/B Trip Explorer", vec![(Key::Enter, "quit")]),
            ModalMenu::new("A/B All Trips Explorer", vec![(Key::Enter, "quit")]),
            ModalMenu::new("Agent Follower", vec![(Key::F, "quit")]),
            ModalMenu::new("Search", vec![(Key::Enter, "quit")]),
            ModalMenu::new("Neighborhood Summaries", vec![(Key::Enter, "quit")]),
            ModalMenu::new(
                "Agent Route Debugger",
                vec![(Key::R, "quit"), (Key::L, "show route for all agents")],
            ),
            ModalMenu::new("Active Traffic Visualizer", vec![(Key::A, "quit")]),
            ModalMenu::new("Object Hider", vec![(Key::K, "unhide everything")]),
            // TODO F1?
            ModalMenu::new("Legend", vec![(Key::L, "quit")]),
            ModalMenu::new(
                "Polygon Debugger",
                vec![
                    (Key::Enter, "quit"),
                    (Key::Dot, "next item"),
                    (Key::Comma, "prev item"),
                    (Key::F, "first item"),
                    (Key::L, "last item"),
                ],
            ),
            ModalMenu::new("Agent Spawner", vec![(Key::Enter, "quit")]),
            // The new exciting things!
            ModalMenu::new(
                "Map Edit Mode",
                vec![
                    (Key::Escape, "quit"),
                    (Key::S, "save edits"),
                    (Key::L, "load different edits"),
                ],
            ),
        ]
    }

    // TODO This hacky wrapper will soon disappear, when UI stops implementing GUI
    fn event(&mut self, ctx: EventCtx) -> EventLoopMode {
        self.new_event(ctx).0
    }

    fn draw(&self, g: &mut GfxCtx) {
        let ctx = DrawCtx {
            cs: &self.state.cs,
            map: &self.state.primary.map,
            draw_map: &self.state.primary.draw_map,
            sim: &self.state.primary.sim,
            hints: &self.hints,
        };
        let mut sample_intersection: Option<String> = None;

        g.clear(self.state.cs.get_def("true background", Color::BLACK));
        g.redraw(&self.state.primary.draw_map.boundary_polygon);

        if g.canvas.cam_zoom < MIN_ZOOM_FOR_DETAIL && !g.is_screencap() {
            // Unzoomed mode
            if self.state.layers.show_areas {
                g.redraw(&self.state.primary.draw_map.draw_all_areas);
            }
            if self.state.layers.show_lanes {
                g.redraw(&self.state.primary.draw_map.draw_all_thick_roads);
            }
            if self.state.layers.show_intersections {
                g.redraw(&self.state.primary.draw_map.draw_all_unzoomed_intersections);
            }
            if self.state.layers.show_buildings {
                g.redraw(&self.state.primary.draw_map.draw_all_buildings);
            }

            // Still show area selection when zoomed out.
            if self.state.primary.current_flags.debug_areas {
                if let Some(ID::Area(id)) = self.state.primary.current_selection {
                    g.draw_polygon(
                        self.state.cs.get("selected"),
                        &fill_to_boundary_polygon(ctx.draw_map.get_a(id).get_outline(&ctx.map)),
                    );
                }
            }

            self.state
                .primary
                .sim
                .draw_unzoomed(g, &self.state.primary.map);
        } else {
            let mut cache = self.state.primary.draw_map.agents.borrow_mut();
            let objects =
                self.get_renderables_back_to_front(g.get_screen_bounds(), &g.prerender, &mut cache);

            let mut drawn_all_buildings = false;
            let mut drawn_all_areas = false;

            for obj in objects {
                match obj.get_id() {
                    ID::Building(_) => {
                        if !drawn_all_buildings {
                            g.redraw(&self.state.primary.draw_map.draw_all_buildings);
                            drawn_all_buildings = true;
                        }
                    }
                    ID::Area(_) => {
                        if !drawn_all_areas {
                            g.redraw(&self.state.primary.draw_map.draw_all_areas);
                            drawn_all_areas = true;
                        }
                    }
                    _ => {}
                };
                let opts = RenderOptions {
                    color: self.state.color_obj(obj.get_id(), &ctx),
                    debug_mode: self.state.layers.debug_mode,
                };
                obj.draw(g, opts, &ctx);

                if self.state.primary.current_selection == Some(obj.get_id()) {
                    g.draw_polygon(
                        self.state.cs.get_def("selected", Color::YELLOW.alpha(0.4)),
                        &fill_to_boundary_polygon(obj.get_outline(&ctx.map)),
                    );
                }

                if g.is_screencap() && sample_intersection.is_none() {
                    if let ID::Intersection(id) = obj.get_id() {
                        sample_intersection = Some(format!("_i{}", id.0));
                    }
                }
            }
        }

        if !g.is_screencap() {
            self.state.draw(g, &ctx);

            // Not happy about cloning, but probably will make the OSD a first-class ezgui concept
            // soon, so meh
            let mut osd = self.hints.osd.clone();
            // TODO Only in some kind of debug mode
            osd.add_line(format!(
                "{} things uploaded, {} things drawn",
                abstutil::prettyprint_usize(g.get_num_uploads()),
                abstutil::prettyprint_usize(g.num_draw_calls),
            ));
            g.draw_blocking_text(&osd, BOTTOM_LEFT);
        }

        if let Some(i) = sample_intersection {
            g.set_screencap_naming_hint(i);
        }
    }

    fn dump_before_abort(&self, canvas: &Canvas) {
        println!(
            "********************************************************************************"
        );
        println!("UI broke! Primary sim:");
        self.state.primary.sim.dump_before_abort();
        if let Some((s, _)) = &self.state.secondary {
            println!("Secondary sim:");
            s.sim.dump_before_abort();
        }

        self.save_editor_state(canvas);
    }

    fn before_quit(&self, canvas: &Canvas) {
        self.save_editor_state(canvas);
        self.state.cs.save();
        println!("Saved color_scheme");
    }

    fn profiling_enabled(&self) -> bool {
        self.state.primary.current_flags.enable_profiler
    }
}

impl UI {
    pub fn new(state: UIState, canvas: &mut Canvas) -> UI {
        match abstutil::read_json::<EditorState>("../editor_state") {
            Ok(ref loaded) if state.primary.map.get_name() == &loaded.map_name => {
                println!("Loaded previous editor_state");
                canvas.cam_x = loaded.cam_x;
                canvas.cam_y = loaded.cam_y;
                canvas.cam_zoom = loaded.cam_zoom;
            }
            _ => {
                println!("Couldn't load editor_state or it's for a different map, so just focusing on an arbitrary building");
                let focus_pt = ID::Building(BuildingID(0))
                    .canonical_point(
                        &state.primary.map,
                        &state.primary.sim,
                        &state.primary.draw_map,
                    )
                    .or_else(|| {
                        ID::Lane(LaneID(0)).canonical_point(
                            &state.primary.map,
                            &state.primary.sim,
                            &state.primary.draw_map,
                        )
                    })
                    .expect("Can't get canonical_point of BuildingID(0) or Road(0)");
                canvas.center_on_map_pt(focus_pt);
            }
        }

        UI {
            state,
            hints: RenderingHints {
                mode: EventLoopMode::InputOnly,
                osd: Text::new(),
                suppress_traffic_signal_details: None,
                hide_turn_icons: HashSet::new(),
            },
        }
    }

    // True if we should pause.
    pub fn new_event(&mut self, mut ctx: EventCtx) -> (EventLoopMode, bool) {
        self.hints = RenderingHints {
            mode: EventLoopMode::InputOnly,
            osd: Text::new(),
            suppress_traffic_signal_details: None,
            hide_turn_icons: HashSet::new(),
        };

        // First update the camera
        ctx.canvas.handle_event(ctx.input);

        // Always handle mouseover
        if !ctx.canvas.is_dragging() && ctx.input.get_moved_mouse().is_some() {
            self.state.primary.current_selection = self.mouseover_something(&ctx);
        }
        if ctx.input.window_lost_cursor() {
            self.state.primary.current_selection = None;
        }

        let mut recalculate_current_selection = false;
        self.state.event(
            &mut ctx,
            &mut self.hints,
            &mut recalculate_current_selection,
        );
        if recalculate_current_selection {
            self.state.primary.current_selection = self.mouseover_something(&ctx);
        }

        ctx.input.populate_osd(&mut self.hints.osd);

        // TODO a plugin should do this, even though it's such a tiny thing
        if self.state.enable_debug_controls {
            if ctx.input.action_chosen("screenshot everything") {
                let bounds = self.state.primary.map.get_bounds();
                assert!(bounds.min_x == 0.0 && bounds.min_y == 0.0);
                self.hints.mode = EventLoopMode::ScreenCaptureEverything {
                    dir: format!(
                        "../data/screenshots/pending_{}",
                        self.state.primary.map.get_name()
                    ),
                    zoom: 3.0,
                    max_x: bounds.max_x,
                    max_y: bounds.max_y,
                };
            }
            if ctx.input.action_chosen("screenshot just this") {
                self.hints.mode = EventLoopMode::ScreenCaptureCurrentShot;
            }
        }

        (
            self.hints.mode.clone(),
            ctx.input.action_chosen("pause game"),
        )
    }

    fn mouseover_something(&self, ctx: &EventCtx) -> Option<ID> {
        // Unzoomed mode. Ignore when debugging areas.
        if ctx.canvas.cam_zoom < MIN_ZOOM_FOR_DETAIL
            && !self.state.primary.current_flags.debug_areas
        {
            return None;
        }

        let pt = ctx.canvas.get_cursor_in_map_space()?;

        let mut cache = self.state.primary.draw_map.agents.borrow_mut();
        let mut objects = self.get_renderables_back_to_front(
            Circle::new(pt, Distance::meters(3.0)).get_bounds(),
            ctx.prerender,
            &mut cache,
        );
        objects.reverse();

        let debug_areas = self.state.primary.current_flags.debug_areas;
        for obj in objects {
            // Don't mouseover areas.
            // TODO Might get fancier rules in the future, so we can't mouseover irrelevant things
            // in intersection editor mode, for example.
            match obj.get_id() {
                ID::Area(_) if !debug_areas => {}
                // Thick roads are only shown when unzoomed, when we don't mouseover at all.
                ID::Road(_) => {}
                _ => {
                    if obj.get_outline(&self.state.primary.map).contains_pt(pt) {
                        return Some(obj.get_id());
                    }
                }
            };
        }
        None
    }

    fn save_editor_state(&self, canvas: &Canvas) {
        let state = EditorState {
            map_name: self.state.primary.map.get_name().clone(),
            cam_x: canvas.cam_x,
            cam_y: canvas.cam_y,
            cam_zoom: canvas.cam_zoom,
        };
        // TODO maybe make state line up with the map, so loading from a new map doesn't break
        abstutil::write_json("../editor_state", &state).expect("Saving editor_state failed");
        println!("Saved editor_state");
    }

    // TODO This could probably belong to DrawMap again, but it's annoying to plumb things that
    // State does, like show_icons_for() and show().
    fn get_renderables_back_to_front<'a>(
        &'a self,
        bounds: Bounds,
        prerender: &Prerender,
        agents: &'a mut AgentCache,
    ) -> Vec<Box<&'a Renderable>> {
        let map = &self.state.primary.map;
        let draw_map = &self.state.primary.draw_map;

        let mut areas: Vec<Box<&Renderable>> = Vec::new();
        let mut lanes: Vec<Box<&Renderable>> = Vec::new();
        let mut roads: Vec<Box<&Renderable>> = Vec::new();
        let mut intersections: Vec<Box<&Renderable>> = Vec::new();
        let mut buildings: Vec<Box<&Renderable>> = Vec::new();
        let mut extra_shapes: Vec<Box<&Renderable>> = Vec::new();
        let mut bus_stops: Vec<Box<&Renderable>> = Vec::new();
        let mut turn_icons: Vec<Box<&Renderable>> = Vec::new();
        let mut agents_on: Vec<Traversable> = Vec::new();

        for id in draw_map.get_matching_objects(bounds) {
            if !self.state.show(id) {
                continue;
            }
            match id {
                ID::Area(id) => areas.push(Box::new(draw_map.get_a(id))),
                ID::Lane(id) => {
                    lanes.push(Box::new(draw_map.get_l(id)));
                    let lane = map.get_l(id);
                    if self.state.show_icons_for(lane.dst_i) {
                        for (t, _) in map.get_next_turns_and_lanes(id, lane.dst_i) {
                            turn_icons.push(Box::new(draw_map.get_t(t.id)));
                        }
                    } else {
                        // TODO Bug: pedestrians on front paths aren't selectable.
                        agents_on.push(Traversable::Lane(id));
                    }
                    for bs in &lane.bus_stops {
                        bus_stops.push(Box::new(draw_map.get_bs(*bs)));
                    }
                }
                ID::Road(id) => {
                    roads.push(Box::new(draw_map.get_r(id)));
                }
                ID::Intersection(id) => {
                    intersections.push(Box::new(draw_map.get_i(id)));
                    for t in &map.get_i(id).turns {
                        if !self.state.show_icons_for(id) {
                            agents_on.push(Traversable::Turn(*t));
                        }
                    }
                }
                // TODO front paths will get drawn over buildings, depending on quadtree order.
                // probably just need to make them go around other buildings instead of having
                // two passes through buildings.
                ID::Building(id) => buildings.push(Box::new(draw_map.get_b(id))),
                ID::ExtraShape(id) => extra_shapes.push(Box::new(draw_map.get_es(id))),

                ID::BusStop(_) | ID::Turn(_) | ID::Car(_) | ID::Pedestrian(_) | ID::Trip(_) => {
                    panic!("{:?} shouldn't be in the quadtree", id)
                }
            }
        }

        // From background to foreground Z-order
        let mut borrows: Vec<Box<&Renderable>> = Vec::new();
        borrows.extend(areas);
        borrows.extend(lanes);
        borrows.extend(roads);
        borrows.extend(intersections);
        borrows.extend(buildings);
        borrows.extend(extra_shapes);
        borrows.extend(bus_stops);
        borrows.extend(turn_icons);

        // Expand all of the Traversables into agents, populating the cache if needed.
        {
            let source = self.state.get_draw_agents();
            let time = source.time();

            for on in &agents_on {
                if !agents.has(time, *on) {
                    let mut list: Vec<Box<Renderable>> = Vec::new();
                    for c in source.get_draw_cars(*on, map).into_iter() {
                        list.push(draw_vehicle(c, map, prerender, &self.state.cs));
                    }
                    for p in source.get_draw_peds(*on, map).into_iter() {
                        list.push(Box::new(DrawPedestrian::new(
                            p,
                            map,
                            prerender,
                            &self.state.cs,
                        )));
                    }
                    agents.put(time, *on, list);
                }
            }
        }

        for on in agents_on {
            for obj in agents.get(on) {
                borrows.push(obj);
            }
        }

        // This is a stable sort.
        borrows.sort_by_key(|r| r.get_zorder());

        borrows
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct EditorState {
    pub map_name: String,
    pub cam_x: f64,
    pub cam_y: f64,
    pub cam_zoom: f64,
}

fn fill_to_boundary_polygon(poly: Polygon) -> Polygon {
    // TODO This looks awful for lanes, oops.
    //PolyLine::make_polygons_for_boundary(poly.points().clone(), Distance::meters(0.5))
    poly
}
