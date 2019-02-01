use crate::colors::ColorScheme;
use abstutil;
//use cpuprofiler;
use crate::objects::{Ctx, RenderingHints, ID};
use crate::render::{RenderOptions, RenderOrder, Renderable};
use crate::state::UIState;
use ezgui::{
    Canvas, Color, EventLoopMode, Folder, GfxCtx, Key, ModalMenu, Prerender, Text, TopMenu,
    UserInput, BOTTOM_LEFT, GUI,
};
use geom::{Bounds, Circle, Distance};
use kml;
use map_model::{BuildingID, LaneID};
use serde_derive::{Deserialize, Serialize};
use sim::GetDrawAgents;
use std::collections::HashSet;
use std::process;

pub struct UI<S: UIState> {
    state: S,
    canvas: Canvas,
    // TODO Not sure why this needs to live here and not in state
    cs: ColorScheme,
}

impl<S: UIState> GUI<RenderingHints> for UI<S> {
    fn top_menu(&self) -> Option<TopMenu> {
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
        Some(TopMenu::new(folders, &self.canvas))
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

    fn event(
        &mut self,
        input: &mut UserInput,
        prerender: &Prerender,
    ) -> (EventLoopMode, RenderingHints) {
        let mut hints = RenderingHints {
            mode: EventLoopMode::InputOnly,
            osd: Text::new(),
            suppress_traffic_signal_details: None,
            hide_turn_icons: HashSet::new(),
        };

        // First update the camera and handle zoom
        let old_zoom = self.canvas.cam_zoom;
        self.canvas.handle_event(input);
        let new_zoom = self.canvas.cam_zoom;
        self.state
            .mut_state()
            .layers
            .handle_zoom(old_zoom, new_zoom);

        // Always handle mouseover
        if !self.canvas.is_dragging() && input.get_moved_mouse().is_some() {
            self.state.mut_state().primary.current_selection = self.mouseover_something();
        }
        if input.window_lost_cursor() {
            self.state.mut_state().primary.current_selection = None;
        }

        let mut recalculate_current_selection = false;
        self.state.event(
            input,
            &mut hints,
            &mut recalculate_current_selection,
            &mut self.cs,
            &mut self.canvas,
            prerender,
        );
        if recalculate_current_selection {
            self.state.mut_state().primary.current_selection = self.mouseover_something();
        }

        // Can do this at any time.
        if input.action_chosen("quit") {
            self.before_quit();
            process::exit(0);
        }

        input.populate_osd(&mut hints.osd);

        // TODO a plugin should do this, even though it's such a tiny thing
        if input.unimportant_key_pressed(Key::F1, "take screenshot") {
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

    fn get_mut_canvas(&mut self) -> &mut Canvas {
        &mut self.canvas
    }

    fn draw(&self, _: &mut GfxCtx, _: &RenderingHints) {}

    fn new_draw(&self, g: &mut GfxCtx, hints: &RenderingHints, screencap: bool) -> Option<String> {
        g.clear(self.cs.get_def("map background", Color::rgb(242, 239, 233)));

        let ctx = Ctx {
            cs: &self.cs,
            map: &self.state.get_state().primary.map,
            draw_map: &self.state.get_state().primary.draw_map,
            canvas: &self.canvas,
            sim: &self.state.get_state().primary.sim,
            hints: &hints,
        };

        let mut sample_intersection: Option<String> = None;
        self.handle_objects(
            self.canvas.get_screen_bounds(),
            RenderOrder::BackToFront,
            |obj| {
                let opts = RenderOptions {
                    color: self.state.get_state().color_obj(obj.get_id(), &ctx),
                    debug_mode: self.state.get_state().layers.debug_mode.is_enabled(),
                    is_selected: self.state.get_state().primary.current_selection
                        == Some(obj.get_id()),
                    // TODO If a ToggleableLayer is currently off, this won't affect it!
                    show_all_detail: screencap,
                };
                obj.draw(g, opts, &ctx);

                if screencap && sample_intersection.is_none() {
                    if let ID::Intersection(id) = obj.get_id() {
                        sample_intersection = Some(format!("_i{}", id.0));
                    }
                }

                true
            },
        );

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
            self.canvas.draw_blocking_text(g, osd, BOTTOM_LEFT);
        }

        sample_intersection
    }

    fn dump_before_abort(&self) {
        error!("********************************************************************************");
        error!("UI broke! Primary sim:");
        self.state.get_state().primary.sim.dump_before_abort();
        if let Some((s, _)) = &self.state.get_state().secondary {
            error!("Secondary sim:");
            s.sim.dump_before_abort();
        }

        self.save_editor_state();
    }

    fn before_quit(&self) {
        self.save_editor_state();
        self.cs.save();
        info!("Saved color_scheme");
        //cpuprofiler::PROFILER.lock().unwrap().stop().unwrap();
    }
}

impl<S: UIState> UI<S> {
    pub fn new(state: S, canvas: Canvas, cs: ColorScheme) -> UI<S> {
        let mut ui = UI { state, canvas, cs };

        match abstutil::read_json::<EditorState>("../editor_state") {
            Ok(ref state) if ui.state.get_state().primary.map.get_name() == &state.map_name => {
                info!("Loaded previous editor_state");
                ui.canvas.cam_x = state.cam_x;
                ui.canvas.cam_y = state.cam_y;
                ui.canvas.cam_zoom = state.cam_zoom;
            }
            _ => {
                warn!("Couldn't load editor_state or it's for a different map, so just focusing on an arbitrary building");
                let focus_pt = ID::Building(BuildingID(0))
                    .canonical_point(
                        &ui.state.get_state().primary.map,
                        &ui.state.get_state().primary.sim,
                        &ui.state.get_state().primary.draw_map,
                    )
                    .or_else(|| {
                        ID::Lane(LaneID(0)).canonical_point(
                            &ui.state.get_state().primary.map,
                            &ui.state.get_state().primary.sim,
                            &ui.state.get_state().primary.draw_map,
                        )
                    })
                    .expect("Can't get canonical_point of BuildingID(0) or Road(0)");
                ui.canvas.center_on_map_pt(focus_pt);
            }
        }

        ui
    }

    fn mouseover_something(&self) -> Option<ID> {
        let pt = self.canvas.get_cursor_in_map_space()?;

        let mut id: Option<ID> = None;

        self.handle_objects(
            Circle::new(pt, Distance::meters(3.0)).get_bounds(),
            RenderOrder::FrontToBack,
            |obj| {
                // Don't mouseover parcels.
                // TODO Might get fancier rules in the future, so we can't mouseover irrelevant things
                // in intersection editor mode, for example.
                match obj.get_id() {
                    ID::Parcel(_) => {}
                    _ => {
                        if obj.contains_pt(pt) {
                            id = Some(obj.get_id());
                            return false;
                        }
                    }
                };
                true
            },
        );

        id
    }

    fn handle_objects<F: FnMut(Box<&Renderable>) -> bool>(
        &self,
        bounds: Bounds,
        order: RenderOrder,
        callback: F,
    ) {
        let state = self.state.get_state();

        let draw_agent_source: &GetDrawAgents = {
            let tt = &state.primary_plugins.time_travel;
            if tt.is_active() {
                tt
            } else {
                &state.primary.sim
            }
        };

        state.primary.draw_map.handle_objects(
            bounds,
            &state.primary.map,
            draw_agent_source,
            state,
            order,
            callback,
        )
    }

    fn save_editor_state(&self) {
        let state = EditorState {
            map_name: self.state.get_state().primary.map.get_name().clone(),
            cam_x: self.canvas.cam_x,
            cam_y: self.canvas.cam_y,
            cam_zoom: self.canvas.cam_zoom,
        };
        // TODO maybe make state line up with the map, so loading from a new map doesn't break
        abstutil::write_json("../editor_state", &state).expect("Saving editor_state failed");
        info!("Saved editor_state");
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct EditorState {
    pub map_name: String,
    pub cam_x: f64,
    pub cam_y: f64,
    pub cam_zoom: f64,
}
