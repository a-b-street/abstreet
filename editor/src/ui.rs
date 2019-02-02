use abstutil;
//use cpuprofiler;
use crate::objects::{Ctx, RenderingHints, ID};
use crate::render::{new_draw_vehicle, DrawPedestrian, RenderOptions, Renderable};
use crate::state::{ShowObjects, UIState};
use ezgui::{
    Canvas, Color, EventCtx, EventLoopMode, Folder, GfxCtx, Key, ModalMenu, Prerender, Text,
    TopMenu, BOTTOM_LEFT, GUI,
};
use geom::{Bounds, Circle, Distance};
use kml;
use map_model::{BuildingID, LaneID, Traversable};
use serde_derive::{Deserialize, Serialize};
use sim::GetDrawAgents;
use std::collections::HashSet;
use std::process;

pub struct UI<S: UIState> {
    state: S,
}

impl<S: UIState> GUI<RenderingHints> for UI<S> {
    fn top_menu(&self, canvas: &Canvas) -> Option<TopMenu> {
        let mut folders = Vec::new();
        folders.push(Folder::new(
            "File",
            vec![
                (Key::Comma, "show log console"),
                (Key::L, "show legend"),
                (Key::Escape, "quit"),
            ],
        ));
        if self.state.get_state().enable_debug_controls {
            folders.push(Folder::new(
                "Debug",
                vec![
                    (Key::C, "find chokepoints"),
                    (Key::I, "validate map geometry"),
                    (Key::Num1, "show/hide buildings"),
                    (Key::Num2, "show/hide intersections"),
                    (Key::Num3, "show/hide lanes"),
                    (Key::Num4, "show/hide parcels"),
                    (Key::Num5, "show/hide areas"),
                    (Key::Num6, "show OSM colors"),
                    (Key::Num7, "show/hide extra shapes"),
                    (Key::Num9, "show/hide all turn icons"),
                    (Key::G, "show/hide geometry debug mode"),
                ],
            ));
        }
        folders.extend(vec![
            Folder::new(
                "Edit",
                vec![
                    (Key::B, "manage A/B tests"),
                    (Key::Num8, "configure colors"),
                    (Key::N, "manage neighborhoods"),
                    (Key::Q, "manage map edits"),
                    (Key::E, "edit roads"),
                    (Key::W, "manage scenarios"),
                ],
            ),
            Folder::new(
                "Simulation",
                vec![
                    (Key::LeftBracket, "slow down sim"),
                    (Key::RightBracket, "speed up sim"),
                    (Key::O, "save sim state"),
                    (Key::Y, "load previous sim state"),
                    (Key::U, "load next sim state"),
                    (Key::Space, "run/pause sim"),
                    (Key::M, "run one step of sim"),
                    (Key::Dot, "show/hide sim info sidepanel"),
                    (Key::T, "start time traveling"),
                    (Key::D, "diff all A/B trips"),
                    (Key::S, "seed the sim with agents"),
                    (Key::LeftAlt, "swap the primary/secondary sim"),
                ],
            ),
            Folder::new(
                "View",
                vec![
                    (Key::Z, "show neighborhood summaries"),
                    (Key::Slash, "search for something"),
                    (Key::A, "show lanes with active traffic"),
                    (Key::J, "warp to an object"),
                ],
            ),
        ]);
        Some(TopMenu::new(folders, canvas))
    }

    fn modal_menus() -> Vec<ModalMenu> {
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
                vec![(Key::S, "save"), (Key::E, "edit"), (Key::I, "instantiate")],
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
                    (Key::Escape, "quit"),
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
                "Geometry Debugger",
                vec![(Key::Enter, "quit"), (Key::N, "see next problem")],
            ),
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
            ModalMenu::new("Neighborhood Summaries", vec![(Key::Z, "quit")]),
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
                ],
            ),
            ModalMenu::new("Agent Spawner", vec![(Key::Enter, "quit")]),
        ]
    }

    fn event(&mut self, mut ctx: EventCtx) -> (EventLoopMode, RenderingHints) {
        let mut hints = RenderingHints {
            mode: EventLoopMode::InputOnly,
            osd: Text::new(),
            suppress_traffic_signal_details: None,
            hide_turn_icons: HashSet::new(),
        };

        // First update the camera and handle zoom
        let old_zoom = ctx.canvas.cam_zoom;
        ctx.canvas.handle_event(ctx.input);
        let new_zoom = ctx.canvas.cam_zoom;
        self.state
            .mut_state()
            .layers
            .handle_zoom(old_zoom, new_zoom);

        // Always handle mouseover
        if !ctx.canvas.is_dragging() && ctx.input.get_moved_mouse().is_some() {
            self.state.mut_state().primary.current_selection = self.mouseover_something(&ctx);
        }
        if ctx.input.window_lost_cursor() {
            self.state.mut_state().primary.current_selection = None;
        }

        let mut recalculate_current_selection = false;
        self.state
            .event(&mut ctx, &mut hints, &mut recalculate_current_selection);
        if recalculate_current_selection {
            self.state.mut_state().primary.current_selection = self.mouseover_something(&ctx);
        }

        // Can do this at any time.
        if ctx.input.action_chosen("quit") {
            self.before_quit(ctx.canvas);
            process::exit(0);
        }

        ctx.input.populate_osd(&mut hints.osd);

        // TODO a plugin should do this, even though it's such a tiny thing
        if ctx
            .input
            .unimportant_key_pressed(Key::F1, "take screenshot")
        {
            let bounds = self.state.get_state().primary.map.get_bounds();
            assert!(bounds.min_x == 0.0 && bounds.min_y == 0.0);
            hints.mode = EventLoopMode::ScreenCaptureEverything {
                zoom: 3.0,
                max_x: bounds.max_x,
                max_y: bounds.max_y,
            };
        }

        (hints.mode, hints)
    }

    fn draw(&self, _: &mut GfxCtx, _: &RenderingHints) {}

    fn new_draw(&self, g: &mut GfxCtx, hints: &RenderingHints, screencap: bool) -> Option<String> {
        let state = self.state.get_state();

        g.clear(
            state
                .cs
                .get_def("map background", Color::rgb(242, 239, 233)),
        );

        let (mut borrows, agents_on) =
            self.get_renderables_back_to_front(g.get_screen_bounds(), &g.prerender());
        let cache = state.primary.draw_map.agents.borrow();
        for on in agents_on {
            for obj in cache.get(on) {
                borrows.push(obj);
            }
        }
        // This is a stable sort.
        borrows.sort_by_key(|r| r.get_zorder());

        let ctx = Ctx {
            cs: &state.cs,
            map: &state.primary.map,
            draw_map: &state.primary.draw_map,
            sim: &state.primary.sim,
            hints: &hints,
        };
        let mut sample_intersection: Option<String> = None;
        for obj in borrows {
            let opts = RenderOptions {
                color: state.color_obj(obj.get_id(), &ctx),
                debug_mode: state.layers.debug_mode.is_enabled(),
                is_selected: state.primary.current_selection == Some(obj.get_id()),
                // TODO If a ToggleableLayer is currently off, this won't affect it!
                show_all_detail: screencap,
            };
            obj.draw(g, opts, &ctx);

            if screencap && sample_intersection.is_none() {
                if let ID::Intersection(id) = obj.get_id() {
                    sample_intersection = Some(format!("_i{}", id.0));
                }
            }
        }

        if !screencap {
            self.state.draw(g, &ctx);

            // Not happy about cloning, but probably will make the OSD a first-class ezgui concept
            // soon, so meh
            let mut osd = hints.osd.clone();
            // TODO Only in some kind of debug mode
            osd.add_line(format!(
                "{} things uploaded, {} things drawn",
                g.num_new_uploads, g.num_draw_calls,
            ));
            g.draw_blocking_text(osd, BOTTOM_LEFT);
        }

        sample_intersection
    }

    fn dump_before_abort(&self, canvas: &Canvas) {
        error!("********************************************************************************");
        error!("UI broke! Primary sim:");
        self.state.get_state().primary.sim.dump_before_abort();
        if let Some((s, _)) = &self.state.get_state().secondary {
            error!("Secondary sim:");
            s.sim.dump_before_abort();
        }

        self.save_editor_state(canvas);
    }

    fn before_quit(&self, canvas: &Canvas) {
        self.save_editor_state(canvas);
        self.state.get_state().cs.save();
        info!("Saved color_scheme");
        //cpuprofiler::PROFILER.lock().unwrap().stop().unwrap();
    }
}

impl<S: UIState> UI<S> {
    pub fn new(state: S, canvas: &mut Canvas) -> UI<S> {
        match abstutil::read_json::<EditorState>("../editor_state") {
            Ok(ref loaded) if state.get_state().primary.map.get_name() == &loaded.map_name => {
                info!("Loaded previous editor_state");
                canvas.cam_x = loaded.cam_x;
                canvas.cam_y = loaded.cam_y;
                canvas.cam_zoom = loaded.cam_zoom;
            }
            _ => {
                warn!("Couldn't load editor_state or it's for a different map, so just focusing on an arbitrary building");
                let focus_pt = ID::Building(BuildingID(0))
                    .canonical_point(
                        &state.get_state().primary.map,
                        &state.get_state().primary.sim,
                        &state.get_state().primary.draw_map,
                    )
                    .or_else(|| {
                        ID::Lane(LaneID(0)).canonical_point(
                            &state.get_state().primary.map,
                            &state.get_state().primary.sim,
                            &state.get_state().primary.draw_map,
                        )
                    })
                    .expect("Can't get canonical_point of BuildingID(0) or Road(0)");
                canvas.center_on_map_pt(focus_pt);
            }
        }

        UI { state }
    }

    fn mouseover_something(&self, ctx: &EventCtx) -> Option<ID> {
        let pt = ctx.canvas.get_cursor_in_map_space()?;

        let (mut borrows, agents_on) = self.get_renderables_back_to_front(
            Circle::new(pt, Distance::meters(3.0)).get_bounds(),
            ctx.prerender,
        );
        let cache = self.state.get_state().primary.draw_map.agents.borrow();
        for on in agents_on {
            for obj in cache.get(on) {
                borrows.push(obj);
            }
        }
        // This is a stable sort.
        borrows.sort_by_key(|r| r.get_zorder());
        borrows.reverse();

        for obj in borrows {
            // Don't mouseover parcels.
            // TODO Might get fancier rules in the future, so we can't mouseover irrelevant things
            // in intersection editor mode, for example.
            match obj.get_id() {
                ID::Parcel(_) => {}
                _ => {
                    if obj.contains_pt(pt) {
                        return Some(obj.get_id());
                    }
                }
            };
        }
        None
    }

    fn save_editor_state(&self, canvas: &Canvas) {
        let state = EditorState {
            map_name: self.state.get_state().primary.map.get_name().clone(),
            cam_x: canvas.cam_x,
            cam_y: canvas.cam_y,
            cam_zoom: canvas.cam_zoom,
        };
        // TODO maybe make state line up with the map, so loading from a new map doesn't break
        abstutil::write_json("../editor_state", &state).expect("Saving editor_state failed");
        info!("Saved editor_state");
    }

    // TODO I guess this technically could go in DrawMap, but we have to pass lots of stuff again.
    fn get_renderables_back_to_front(
        &self,
        bounds: Bounds,
        prerender: &Prerender,
    ) -> (Vec<Box<&Renderable>>, Vec<Traversable>) {
        let state = self.state.get_state();
        let map = &state.primary.map;
        let draw_map = &state.primary.draw_map;

        let mut areas: Vec<Box<&Renderable>> = Vec::new();
        let mut parcels: Vec<Box<&Renderable>> = Vec::new();
        let mut lanes: Vec<Box<&Renderable>> = Vec::new();
        let mut intersections: Vec<Box<&Renderable>> = Vec::new();
        let mut buildings: Vec<Box<&Renderable>> = Vec::new();
        let mut extra_shapes: Vec<Box<&Renderable>> = Vec::new();
        let mut bus_stops: Vec<Box<&Renderable>> = Vec::new();
        let mut turn_icons: Vec<Box<&Renderable>> = Vec::new();
        let mut agents_on: Vec<Traversable> = Vec::new();

        for id in draw_map.get_matching_objects(bounds) {
            if !state.show(id) {
                continue;
            }
            match id {
                ID::Area(id) => areas.push(Box::new(draw_map.get_a(id))),
                ID::Parcel(id) => parcels.push(Box::new(draw_map.get_p(id))),
                ID::Lane(id) => {
                    lanes.push(Box::new(draw_map.get_l(id)));
                    if !state.show_icons_for(map.get_l(id).dst_i) {
                        agents_on.push(Traversable::Lane(id));
                    }
                }
                ID::Intersection(id) => {
                    intersections.push(Box::new(draw_map.get_i(id)));
                    for t in &map.get_i(id).turns {
                        if state.show_icons_for(id) {
                            turn_icons.push(Box::new(draw_map.get_t(*t)));
                        } else {
                            agents_on.push(Traversable::Turn(*t));
                        }
                    }
                }
                // TODO front paths will get drawn over buildings, depending on quadtree order.
                // probably just need to make them go around other buildings instead of having
                // two passes through buildings.
                ID::Building(id) => buildings.push(Box::new(draw_map.get_b(id))),
                ID::ExtraShape(id) => extra_shapes.push(Box::new(draw_map.get_es(id))),
                ID::BusStop(id) => bus_stops.push(Box::new(draw_map.get_bs(id))),

                ID::Turn(_) | ID::Car(_) | ID::Pedestrian(_) | ID::Trip(_) => {
                    panic!("{:?} shouldn't be in the quadtree", id)
                }
            }
        }

        // From background to foreground Z-order
        let mut borrows: Vec<Box<&Renderable>> = Vec::new();
        borrows.extend(areas);
        borrows.extend(parcels);
        borrows.extend(lanes);
        borrows.extend(intersections);
        borrows.extend(buildings);
        borrows.extend(extra_shapes);
        borrows.extend(bus_stops);
        borrows.extend(turn_icons);

        // Make sure agents are cached, but we can't actually return the references to them here,
        // since the RefCell borrow can't outlive this function.
        {
            let sim: &GetDrawAgents = {
                let tt = &state.primary_plugins.time_travel;
                if tt.is_active() {
                    tt
                } else {
                    &state.primary.sim
                }
            };
            let tick = sim.tick();
            let mut agents = draw_map.agents.borrow_mut();

            for on in &agents_on {
                if !agents.has(tick, *on) {
                    let mut list: Vec<Box<Renderable>> = Vec::new();
                    for c in sim.get_draw_cars(*on, map).into_iter() {
                        list.push(new_draw_vehicle(c, map, prerender, &state.cs));
                    }
                    for p in sim.get_draw_peds(*on, map).into_iter() {
                        list.push(Box::new(DrawPedestrian::new_new(
                            p, map, prerender, &state.cs,
                        )));
                    }
                    agents.put(tick, *on, list);
                }
            }
        }

        (borrows, agents_on)
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct EditorState {
    pub map_name: String,
    pub cam_x: f64,
    pub cam_y: f64,
    pub cam_zoom: f64,
}
