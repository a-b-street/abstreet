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
use map_model::{BuildingID, IntersectionID, LaneID, Traversable};
use serde_derive::{Deserialize, Serialize};
use sim::GetDrawAgents;
use std::collections::{HashMap, HashSet};

// TODO Collapse stuff!
pub struct UI {
    pub hints: RenderingHints,
    pub state: UIState,
}

impl GUI for UI {
    fn top_menu(&self, canvas: &Canvas) -> Option<TopMenu> {
        let mut folders = Vec::new();
        folders.push(Folder::new("File", vec![(Some(Key::Escape), "pause game")]));
        if self.state.enable_debug_controls {
            folders.push(Folder::new(
                "Debug",
                vec![(Some(Key::F1), "screenshot just this")],
            ));
        }
        folders.extend(vec![
            Folder::new(
                "Edit",
                vec![
                    (Some(Key::B), "manage A/B tests"),
                    (None, "configure colors"),
                    (Some(Key::N), "manage neighborhoods"),
                    (Some(Key::W), "manage scenarios"),
                ],
            ),
            Folder::new("Simulation", vec![(Some(Key::D), "diff all A/B trips")]),
            Folder::new(
                "View",
                vec![
                    (None, "show neighborhood summaries"),
                    (Some(Key::J), "warp to an object"),
                ],
            ),
        ]);
        Some(TopMenu::new(folders, canvas))
    }

    fn modal_menus(&self) -> Vec<ModalMenu> {
        vec![
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
            ModalMenu::new(
                "Color Picker",
                vec![(Key::Backspace, "revert"), (Key::Enter, "finalize")],
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
            ModalMenu::new("A/B Trip Explorer", vec![(Key::Enter, "quit")]),
            ModalMenu::new("A/B All Trips Explorer", vec![(Key::Enter, "quit")]),
            ModalMenu::new("Neighborhood Summaries", vec![(Key::Enter, "quit")]),
            // The new exciting things!
            ModalMenu::new(
                "Map Edit Mode",
                vec![
                    (Key::Escape, "quit"),
                    (Key::S, "save edits"),
                    (Key::L, "load different edits"),
                ],
            ),
            ModalMenu::new(
                "Stop Sign Editor",
                vec![(Key::Escape, "quit"), (Key::R, "reset to default")],
            ),
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
                "Sandbox Mode",
                vec![
                    (Key::Escape, "quit"),
                    (Key::LeftBracket, "slow down sim"),
                    (Key::RightBracket, "speed up sim"),
                    (Key::O, "save sim state"),
                    (Key::Y, "load previous sim state"),
                    (Key::U, "load next sim state"),
                    (Key::Space, "run/pause sim"),
                    (Key::M, "run one step of sim"),
                    (Key::X, "reset sim"),
                    (Key::S, "seed the sim with agents"),
                    // TODO Strange to always have this. Really it's a case of stacked modal?
                    (Key::F, "stop following agent"),
                    (Key::R, "stop showing agent's route"),
                    // TODO This should probably be a debug thing instead
                    (Key::L, "show/hide route for all agents"),
                    (Key::A, "show/hide active traffic"),
                    (Key::T, "start time traveling"),
                ],
            ),
            ModalMenu::new("Agent Spawner", vec![(Key::Escape, "quit")]),
            ModalMenu::new(
                "Time Traveler",
                vec![
                    (Key::Escape, "quit"),
                    (Key::Comma, "rewind"),
                    (Key::Dot, "forwards"),
                ],
            ),
            ModalMenu::new(
                "Debug Mode",
                vec![
                    (Key::Escape, "quit"),
                    (Key::C, "show/hide chokepoints"),
                    (Key::O, "clear original roads shown"),
                    (Key::K, "unhide everything"),
                    (Key::Num1, "show/hide buildings"),
                    (Key::Num2, "show/hide intersections"),
                    (Key::Num3, "show/hide lanes"),
                    (Key::Num4, "show/hide areas"),
                    (Key::Num5, "show/hide extra shapes"),
                    (Key::Num6, "show/hide geometry debug mode"),
                    (Key::F1, "screenshot everything"),
                    (Key::Slash, "search OSM metadata"),
                    (Key::M, "clear OSM search results"),
                ],
            ),
            ModalMenu::new(
                "Polygon Debugger",
                vec![
                    (Key::Escape, "quit"),
                    (Key::Dot, "next item"),
                    (Key::Comma, "prev item"),
                    (Key::F, "first item"),
                    (Key::L, "last item"),
                ],
            ),
        ]
    }

    // TODO This hacky wrapper will soon disappear, when UI stops implementing GUI
    fn event(&mut self, ctx: &mut EventCtx) -> EventLoopMode {
        self.new_event(ctx).0
    }

    fn draw(&self, g: &mut GfxCtx) {
        self.new_draw(
            g,
            None,
            HashMap::new(),
            &self.state.primary.sim,
            &ShowEverything::new(),
        )
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
    pub fn new_event(&mut self, ctx: &mut EventCtx) -> (EventLoopMode, bool) {
        self.hints = RenderingHints {
            mode: EventLoopMode::InputOnly,
            osd: Text::new(),
            suppress_traffic_signal_details: None,
            hide_turn_icons: HashSet::new(),
        };

        // First update the camera
        ctx.canvas.handle_event(ctx.input);

        // Always handle mouseover
        self.state.primary.current_selection =
            self.handle_mouseover(ctx, None, &self.state.primary.sim, &ShowEverything::new());

        let mut recalculate_current_selection = false;
        self.state
            .event(ctx, &mut self.hints, &mut recalculate_current_selection);
        if recalculate_current_selection {
            self.state.primary.current_selection = self.mouseover_something(
                &ctx,
                None,
                &self.state.primary.sim,
                &ShowEverything::new(),
            );
        }

        ctx.input.populate_osd(&mut self.hints.osd);

        // TODO a plugin should do this, even though it's such a tiny thing
        if self.state.enable_debug_controls {
            if ctx.input.action_chosen("screenshot just this") {
                self.hints.mode = EventLoopMode::ScreenCaptureCurrentShot;
            }
        }

        (
            self.hints.mode.clone(),
            ctx.input.action_chosen("pause game"),
        )
    }

    pub fn new_draw(
        &self,
        g: &mut GfxCtx,
        show_turn_icons_for: Option<IntersectionID>,
        override_color: HashMap<ID, Color>,
        source: &GetDrawAgents,
        show_objs: &ShowObject,
    ) {
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
            let layers = show_objs.layers();
            if layers.show_areas {
                g.redraw(&self.state.primary.draw_map.draw_all_areas);
            }
            if layers.show_lanes {
                g.redraw(&self.state.primary.draw_map.draw_all_thick_roads);
            }
            if layers.show_intersections {
                g.redraw(&self.state.primary.draw_map.draw_all_unzoomed_intersections);
            }
            if layers.show_buildings {
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
            let objects = self.get_renderables_back_to_front(
                g.get_screen_bounds(),
                &g.prerender,
                &mut cache,
                show_turn_icons_for,
                source,
                show_objs,
            );

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
                    color: override_color
                        .get(&obj.get_id())
                        .cloned()
                        .or_else(|| self.state.color_obj(obj.get_id(), &ctx)),
                    debug_mode: show_objs.layers().geom_debug_mode,
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

    // Because we have to sometimes borrow part of self for GetDrawAgents, this just returns the
    // Option<ID> that the caller should assign. When this monolithic UI nonsense is dismantled,
    // this weirdness goes away.
    pub fn handle_mouseover(
        &self,
        ctx: &mut EventCtx,
        show_turn_icons_for: Option<IntersectionID>,
        source: &GetDrawAgents,
        show_objs: &ShowObject,
    ) -> Option<ID> {
        if !ctx.canvas.is_dragging() && ctx.input.get_moved_mouse().is_some() {
            return self.mouseover_something(&ctx, show_turn_icons_for, source, show_objs);
        }
        if ctx.input.window_lost_cursor() {
            return None;
        }
        self.state.primary.current_selection
    }

    fn mouseover_something(
        &self,
        ctx: &EventCtx,
        show_turn_icons_for: Option<IntersectionID>,
        source: &GetDrawAgents,
        show_objs: &ShowObject,
    ) -> Option<ID> {
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
            show_turn_icons_for,
            source,
            show_objs,
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
        show_turn_icons_for: Option<IntersectionID>,
        source: &GetDrawAgents,
        show_objs: &ShowObject,
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
            if !show_objs.show(id) {
                continue;
            }
            match id {
                ID::Area(id) => areas.push(Box::new(draw_map.get_a(id))),
                ID::Lane(id) => {
                    lanes.push(Box::new(draw_map.get_l(id)));
                    let lane = map.get_l(id);
                    if show_turn_icons_for == Some(lane.dst_i) {
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
                        if show_turn_icons_for != Some(id) {
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

pub struct ShowLayers {
    pub show_buildings: bool,
    pub show_intersections: bool,
    pub show_lanes: bool,
    pub show_areas: bool,
    pub show_extra_shapes: bool,
    pub geom_debug_mode: bool,
}

impl ShowLayers {
    pub fn new() -> ShowLayers {
        ShowLayers {
            show_buildings: true,
            show_intersections: true,
            show_lanes: true,
            show_areas: true,
            show_extra_shapes: true,
            geom_debug_mode: false,
        }
    }
}

pub trait ShowObject {
    fn show(&self, obj: ID) -> bool;
    fn layers(&self) -> &ShowLayers;
}

pub struct ShowEverything {
    layers: ShowLayers,
}

impl ShowEverything {
    pub fn new() -> ShowEverything {
        ShowEverything {
            layers: ShowLayers::new(),
        }
    }
}

impl ShowObject for ShowEverything {
    fn show(&self, _: ID) -> bool {
        true
    }

    fn layers(&self) -> &ShowLayers {
        &self.layers
    }
}
