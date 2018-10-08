// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

// TODO this should just be a way to handle interactions between plugins

use abstutil;
use colors::{ColorScheme, Colors};
use control::ControlMap;
use ezgui::{
    Canvas, Color, EventLoopMode, GfxCtx, Text, ToggleableLayer, UserInput, BOTTOM_LEFT, GUI,
};
use flame;
use kml;
use map_model::{IntersectionID, Map};
use objects::{Ctx, DEBUG_LAYERS, ID, ROOT_MENU};
use piston::input::Key;
use plugins::a_b_tests::ABTestManager;
use plugins::classification::OsmClassifier;
use plugins::color_picker::ColorPicker;
use plugins::debug_objects::DebugObjectsState;
use plugins::draw_neighborhoods::DrawNeighborhoodState;
use plugins::floodfill::Floodfiller;
use plugins::follow::FollowState;
use plugins::geom_validation::Validator;
use plugins::hider::Hider;
use plugins::logs::DisplayLogs;
use plugins::map_edits::EditsManager;
use plugins::road_editor::RoadEditor;
use plugins::scenarios::ScenarioManager;
use plugins::search::SearchState;
use plugins::show_route::ShowRouteState;
use plugins::sim_controls::SimController;
use plugins::steep::SteepnessVisualizer;
use plugins::stop_sign_editor::StopSignEditor;
use plugins::traffic_signal_editor::TrafficSignalEditor;
use plugins::turn_cycler::TurnCyclerState;
use plugins::warp::WarpState;
use plugins::Colorizer;
use render::{DrawMap, RenderOptions};
use sim;
use sim::{Sim, SimFlags};
use std::process;

// TODO ideally these would be tuned kind of dynamically based on rendering speed
const MIN_ZOOM_FOR_LANES: f64 = 0.15;
const MIN_ZOOM_FOR_PARCELS: f64 = 1.0;
const MIN_ZOOM_FOR_MOUSEOVER: f64 = 4.0;

// Necessary so we can iterate over and run the plugins, which mutably borrow UI.
pub struct UIWrapper {
    ui: UI,
    plugins: Vec<Box<Fn(PluginCtx) -> bool>>,
}

impl GUI for UIWrapper {
    fn event(&mut self, input: UserInput, osd: &mut Text) -> EventLoopMode {
        self.ui.event(input, osd, &self.plugins)
    }

    fn get_mut_canvas(&mut self) -> &mut Canvas {
        &mut self.ui.canvas
    }

    fn draw(&self, g: &mut GfxCtx, osd: Text) {
        self.ui.draw(g, osd);
    }
}

impl UIWrapper {
    // nit: lots of this logic could live in UI, if it mattered
    pub fn new(flags: SimFlags, kml: Option<String>) -> UIWrapper {
        // Do this first, so anything logged by sim::load isn't lost.
        let logs = DisplayLogs::new();

        let mut ui = UI {
            primary: PerMapUI::new(flags, &kml),
            secondary: None,

            layers: ToggleableLayers::new(),
            search_state: SearchState::Empty,
            warp: WarpState::Empty,
            osm_classifier: OsmClassifier::new(),
            sim_ctrl: SimController::new(),
            color_picker: ColorPicker::new(),
            ab_test_manager: ABTestManager::new(),
            logs,

            active_plugin: None,

            canvas: Canvas::new(),
            cs: ColorScheme::load("color_scheme").unwrap(),

            kml,
        };

        match abstutil::read_json::<EditorState>("editor_state") {
            Ok(ref state) if ui.primary.map.get_name().to_string() == state.map_name => {
                info!("Loaded previous editor_state");
                ui.canvas.cam_x = state.cam_x;
                ui.canvas.cam_y = state.cam_y;
                ui.canvas.cam_zoom = state.cam_zoom;
            }
            _ => {
                warn!("Couldn't load editor_state or it's for a different map, so just centering initial view");
                ui.canvas.center_on_map_pt(ui.primary.draw_map.center_pt);
            }
        }

        let new_zoom = ui.canvas.cam_zoom;
        for layer in ui.toggleable_layers().into_iter() {
            layer.handle_zoom(-1.0, new_zoom);
        }

        UIWrapper {
            ui,
            plugins: vec![
                Box::new(|ctx| {
                    let layer_changed = {
                        let mut changed = false;
                        for layer in ctx.ui.toggleable_layers().into_iter() {
                            if layer.event(ctx.input) {
                                changed = true;
                                break;
                            }
                        }
                        changed
                    };
                    if layer_changed {
                        ctx.ui.primary.current_selection = ctx.ui.mouseover_something();
                        true
                    } else {
                        false
                    }
                }),
                Box::new(|ctx| {
                    ctx.ui.primary.traffic_signal_editor.event(
                        ctx.input,
                        &ctx.ui.primary.map,
                        &mut ctx.ui.primary.control_map,
                        ctx.ui.primary.current_selection,
                    )
                }),
                Box::new(|ctx| {
                    ctx.ui.primary.stop_sign_editor.event(
                        ctx.input,
                        &ctx.ui.primary.map,
                        &mut ctx.ui.primary.control_map,
                        ctx.ui.primary.current_selection,
                    )
                }),
                Box::new(|ctx| {
                    ctx.ui.primary.road_editor.event(
                        ctx.input,
                        ctx.ui.primary.current_selection,
                        &mut ctx.ui.primary.map,
                        &mut ctx.ui.primary.draw_map,
                        &ctx.ui.primary.control_map,
                        &mut ctx.ui.primary.sim,
                    )
                }),
                Box::new(|ctx| ctx.ui.search_state.event(ctx.input)),
                Box::new(|ctx| {
                    ctx.ui.warp.event(
                        ctx.input,
                        &ctx.ui.primary.map,
                        &ctx.ui.primary.sim,
                        &mut ctx.ui.canvas,
                        &mut ctx.ui.primary.current_selection,
                    )
                }),
                Box::new(|ctx| {
                    ctx.ui.primary.follow.event(
                        ctx.input,
                        &ctx.ui.primary.map,
                        &ctx.ui.primary.sim,
                        &mut ctx.ui.canvas,
                        ctx.ui.primary.current_selection,
                    )
                }),
                Box::new(|ctx| {
                    ctx.ui.primary.show_route.event(
                        ctx.input,
                        &ctx.ui.primary.sim,
                        ctx.ui.primary.current_selection,
                    )
                }),
                Box::new(|ctx| {
                    ctx.ui
                        .color_picker
                        .event(ctx.input, &mut ctx.ui.canvas, &mut ctx.ui.cs)
                }),
                Box::new(|ctx| ctx.ui.primary.steepness_viz.event(ctx.input)),
                Box::new(|ctx| ctx.ui.osm_classifier.event(ctx.input)),
                Box::new(|ctx| {
                    ctx.ui
                        .primary
                        .hider
                        .event(ctx.input, &mut ctx.ui.primary.current_selection)
                }),
                Box::new(|ctx| {
                    ctx.ui.primary.debug_objects.event(
                        ctx.ui.primary.current_selection,
                        ctx.input,
                        &ctx.ui.primary.map,
                        &mut ctx.ui.primary.sim,
                        &ctx.ui.primary.control_map,
                    )
                }),
                Box::new(|ctx| {
                    ctx.ui.primary.floodfiller.event(
                        &ctx.ui.primary.map,
                        ctx.input,
                        ctx.ui.primary.current_selection,
                    )
                }),
                Box::new(|ctx| {
                    ctx.ui.primary.geom_validator.event(
                        ctx.input,
                        &mut ctx.ui.canvas,
                        &ctx.ui.primary.map,
                        &ctx.ui.primary.draw_map,
                    )
                }),
                Box::new(|ctx| {
                    ctx.ui
                        .primary
                        .turn_cycler
                        .event(ctx.input, ctx.ui.primary.current_selection)
                }),
                Box::new(|ctx| {
                    ctx.ui.primary.draw_neighborhoods.event(
                        ctx.input,
                        &ctx.ui.canvas,
                        &ctx.ui.primary.map,
                        ctx.osd,
                    )
                }),
                Box::new(|ctx| {
                    ctx.ui.primary.scenarios.event(
                        ctx.input,
                        &ctx.ui.primary.map,
                        &mut ctx.ui.primary.sim,
                    )
                }),
                Box::new(|ctx| {
                    let (active, new_primary) = ctx.ui.primary.edits_manager.event(
                        ctx.input,
                        &ctx.ui.primary.map,
                        &ctx.ui.primary.control_map,
                        &ctx.ui.primary.road_editor,
                        &mut ctx.ui.primary.current_flags,
                        &ctx.ui.kml,
                    );
                    if new_primary.is_some() {
                        ctx.ui.primary = new_primary.unwrap();
                    }
                    active
                }),
                Box::new(|ctx| {
                    let (active, new_ui) =
                        ctx.ui
                            .ab_test_manager
                            .event(ctx.input, &ctx.ui.primary.map, &ctx.ui.kml);
                    if let Some((new_primary, new_secondary)) = new_ui {
                        ctx.ui.primary = new_primary;
                        ctx.ui.secondary = Some(new_secondary);
                    }
                    active
                }),
                Box::new(|ctx| ctx.ui.logs.event(ctx.input)),
            ],
        }
    }
}

// All of the state that's bound to a specific map+edit has to live here.
// TODO How can we arrange the code so that we statically know that we don't pass anything from UI
// to something in PerMapUI?
pub struct PerMapUI {
    map: Map,
    draw_map: DrawMap,
    control_map: ControlMap,
    sim: Sim,

    current_selection: Option<ID>,
    current_flags: SimFlags,

    // Anything that holds onto any kind of ID has to live here!
    hider: Hider,
    debug_objects: DebugObjectsState,
    follow: FollowState,
    show_route: ShowRouteState,
    floodfiller: Floodfiller,
    steepness_viz: SteepnessVisualizer,
    traffic_signal_editor: TrafficSignalEditor,
    stop_sign_editor: StopSignEditor,
    road_editor: RoadEditor,
    geom_validator: Validator,
    turn_cycler: TurnCyclerState,
    draw_neighborhoods: DrawNeighborhoodState,
    scenarios: ScenarioManager,
    edits_manager: EditsManager,
}

impl PerMapUI {
    pub fn new(flags: SimFlags, kml: &Option<String>) -> PerMapUI {
        flame::start("setup");
        let (map, control_map, sim) = sim::load(flags.clone(), Some(sim::Tick::from_seconds(30)));
        let extra_shapes = if let Some(path) = kml {
            kml::load(&path, &map.get_gps_bounds()).expect("Couldn't load extra KML shapes")
        } else {
            Vec::new()
        };

        flame::start("draw_map");
        let draw_map = DrawMap::new(&map, &control_map, extra_shapes);
        flame::end("draw_map");

        flame::end("setup");
        flame::dump_stdout();

        let steepness_viz = SteepnessVisualizer::new(&map);
        let road_editor = RoadEditor::new(map.get_road_edits().clone());

        PerMapUI {
            map,
            draw_map,
            control_map,
            sim,

            current_selection: None,
            current_flags: flags,

            hider: Hider::new(),
            debug_objects: DebugObjectsState::new(),
            follow: FollowState::Empty,
            show_route: ShowRouteState::Empty,
            floodfiller: Floodfiller::new(),
            steepness_viz,
            traffic_signal_editor: TrafficSignalEditor::new(),
            stop_sign_editor: StopSignEditor::new(),
            road_editor,
            geom_validator: Validator::new(),
            turn_cycler: TurnCyclerState::new(),
            draw_neighborhoods: DrawNeighborhoodState::new(),
            scenarios: ScenarioManager::new(),
            edits_manager: EditsManager::new(),
        }
    }
}

struct UI {
    primary: PerMapUI,
    // When running an A/B test, this is populated too.
    secondary: Option<PerMapUI>,

    layers: ToggleableLayers,
    search_state: SearchState,
    warp: WarpState,
    osm_classifier: OsmClassifier,
    // TODO This one has per-sim state right now, but soon will understand how to handle two sims.
    sim_ctrl: SimController,
    color_picker: ColorPicker,
    ab_test_manager: ABTestManager,
    logs: DisplayLogs,

    // An index into UIWrapper.plugins.
    active_plugin: Option<usize>,

    canvas: Canvas,
    cs: ColorScheme,

    // Remember this to support loading a new PerMapUI
    kml: Option<String>,
}

impl UI {
    fn toggleable_layers(&mut self) -> Vec<&mut ToggleableLayer> {
        vec![
            &mut self.layers.show_lanes,
            &mut self.layers.show_buildings,
            &mut self.layers.show_intersections,
            &mut self.layers.show_parcels,
            &mut self.layers.show_extra_shapes,
            &mut self.layers.show_all_turn_icons,
            &mut self.layers.debug_mode,
        ]
    }

    fn mouseover_something(&self) -> Option<ID> {
        let pt = self.canvas.get_cursor_in_map_space();

        let (statics, dynamics) = self.primary.draw_map.get_objects_onscreen(
            self.canvas.get_screen_bbox(),
            &self.primary.hider,
            &self.primary.map,
            &self.primary.sim,
            &self.layers,
            self,
        );
        // Check front-to-back
        for obj in dynamics.into_iter() {
            if obj.contains_pt(pt) {
                return Some(obj.get_id());
            }
        }
        for obj in statics.into_iter().rev() {
            if obj.contains_pt(pt) {
                return Some(obj.get_id());
            }
        }

        None
    }

    fn event(
        &mut self,
        mut input: UserInput,
        osd: &mut Text,
        plugins: &Vec<Box<Fn(PluginCtx) -> bool>>,
    ) -> EventLoopMode {
        // First update the camera and handle zoom
        let old_zoom = self.canvas.cam_zoom;
        self.canvas.handle_event(&mut input);
        let new_zoom = self.canvas.cam_zoom;
        for layer in self.toggleable_layers().into_iter() {
            layer.handle_zoom(old_zoom, new_zoom);
        }

        // Always handle mouseover
        if old_zoom >= MIN_ZOOM_FOR_MOUSEOVER && new_zoom < MIN_ZOOM_FOR_MOUSEOVER {
            self.primary.current_selection = None;
        }
        if !self.canvas.is_dragging()
            && input.get_moved_mouse().is_some()
            && new_zoom >= MIN_ZOOM_FOR_MOUSEOVER
        {
            self.primary.current_selection = self.mouseover_something();
        }

        // TODO Normally we'd return InputOnly here if there was an active plugin, but actually, we
        // want some keys to always be pressable (sim controller stuff, quitting the game?)

        // If there's an active plugin, just run it.
        if let Some(idx) = self.active_plugin {
            if !plugins[idx](PluginCtx {
                ui: self,
                input: &mut input,
                osd,
            }) {
                self.active_plugin = None;
            }
        } else {
            // Run each plugin, short-circuiting if the plugin claimed it was active.
            for (idx, plugin) in plugins.iter().enumerate() {
                if plugin(PluginCtx {
                    ui: self,
                    input: &mut input,
                    osd,
                }) {
                    self.active_plugin = Some(idx);
                    break;
                }
            }
        }

        if input.unimportant_key_pressed(Key::Escape, ROOT_MENU, "quit") {
            let state = EditorState {
                map_name: self.primary.map.get_name().clone(),
                cam_x: self.canvas.cam_x,
                cam_y: self.canvas.cam_y,
                cam_zoom: self.canvas.cam_zoom,
            };
            // TODO maybe make state line up with the map, so loading from a new map doesn't break
            abstutil::write_json("editor_state", &state).expect("Saving editor_state failed");
            abstutil::write_json("color_scheme", &self.cs).expect("Saving color_scheme failed");
            info!("Saved editor_state and color_scheme");
            process::exit(0);
        }

        // Sim controller plugin is kind of always active? If nothing else ran, let it use keys.
        let result = self.sim_ctrl.event(
            &mut input,
            &self.primary.map,
            &self.primary.control_map,
            &mut self.primary.sim,
            self.primary.current_selection,
            osd,
        );
        input.populate_osd(osd);
        result
    }

    fn draw(&self, g: &mut GfxCtx, osd: Text) {
        g.clear(self.cs.get(Colors::Background));

        let (statics, dynamics) = self.primary.draw_map.get_objects_onscreen(
            self.canvas.get_screen_bbox(),
            &self.primary.hider,
            &self.primary.map,
            &self.primary.sim,
            &self.layers,
            self,
        );
        for obj in statics.into_iter() {
            let opts = RenderOptions {
                color: self.color_obj(obj.get_id()),
                cam_zoom: self.canvas.cam_zoom,
                debug_mode: self.layers.debug_mode.is_enabled(),
            };
            obj.draw(
                g,
                opts,
                Ctx {
                    cs: &self.cs,
                    map: &self.primary.map,
                    control_map: &self.primary.control_map,
                    canvas: &self.canvas,
                    sim: &self.primary.sim,
                },
            );
        }
        for obj in dynamics.into_iter() {
            let opts = RenderOptions {
                color: self.color_obj(obj.get_id()),
                cam_zoom: self.canvas.cam_zoom,
                debug_mode: self.layers.debug_mode.is_enabled(),
            };
            obj.draw(
                g,
                opts,
                Ctx {
                    cs: &self.cs,
                    map: &self.primary.map,
                    control_map: &self.primary.control_map,
                    canvas: &self.canvas,
                    sim: &self.primary.sim,
                },
            );
        }

        // TODO Only if active?
        self.primary.turn_cycler.draw(
            &self.primary.map,
            &self.primary.draw_map,
            &self.primary.control_map,
            self.primary.sim.time,
            &self.cs,
            g,
        );
        self.primary.debug_objects.draw(
            &self.primary.map,
            &self.canvas,
            &self.primary.draw_map,
            &self.primary.sim,
            g,
        );
        self.color_picker.draw(&self.canvas, g);
        self.primary.draw_neighborhoods.draw(g, &self.canvas);
        self.primary.scenarios.draw(g, &self.canvas);
        self.primary.edits_manager.draw(g, &self.canvas);
        self.ab_test_manager.draw(g, &self.canvas);
        self.logs.draw(g, &self.canvas);
        self.search_state.draw(g, &self.canvas);
        self.warp.draw(g, &self.canvas);
        self.sim_ctrl.draw(g, &self.canvas);

        self.canvas.draw_text(g, osd, BOTTOM_LEFT);
    }

    fn color_obj(&self, id: ID) -> Option<Color> {
        if Some(id) == self.primary.current_selection {
            return Some(self.cs.get(Colors::Selected));
        }

        if let Some(p) = self.get_active_plugin() {
            if let Some(c) = p.color_for(
                id,
                Ctx {
                    cs: &self.cs,
                    map: &self.primary.map,
                    control_map: &self.primary.control_map,
                    canvas: &self.canvas,
                    sim: &self.primary.sim,
                },
            ) {
                return Some(c);
            }
        }

        None
    }

    fn get_active_plugin(&self) -> Option<Box<&Colorizer>> {
        let idx = self.active_plugin?;
        // Match instead of array, because can't move the Box out of the temporary vec. :\
        // This must line up with the list of plugins in UIWrapper::new.
        match idx {
            // The first plugin is all the ToggleableLayers, which doesn't implement Colorizer.
            0 => None,
            1 => Some(Box::new(&self.primary.traffic_signal_editor)),
            2 => Some(Box::new(&self.primary.stop_sign_editor)),
            3 => Some(Box::new(&self.primary.road_editor)),
            4 => Some(Box::new(&self.search_state)),
            5 => Some(Box::new(&self.warp)),
            6 => Some(Box::new(&self.primary.follow)),
            7 => Some(Box::new(&self.primary.show_route)),
            8 => Some(Box::new(&self.color_picker)),
            9 => Some(Box::new(&self.primary.steepness_viz)),
            10 => Some(Box::new(&self.osm_classifier)),
            11 => Some(Box::new(&self.primary.hider)),
            12 => Some(Box::new(&self.primary.debug_objects)),
            13 => Some(Box::new(&self.primary.floodfiller)),
            14 => Some(Box::new(&self.primary.geom_validator)),
            15 => Some(Box::new(&self.primary.turn_cycler)),
            16 => Some(Box::new(&self.primary.draw_neighborhoods)),
            17 => Some(Box::new(&self.primary.scenarios)),
            18 => Some(Box::new(&self.primary.edits_manager)),
            19 => Some(Box::new(&self.ab_test_manager)),
            20 => Some(Box::new(&self.logs)),
            _ => panic!("Active plugin {} is too high", idx),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct EditorState {
    pub map_name: String,
    pub cam_x: f64,
    pub cam_y: f64,
    pub cam_zoom: f64,
}

pub struct ToggleableLayers {
    pub show_lanes: ToggleableLayer,
    pub show_buildings: ToggleableLayer,
    pub show_intersections: ToggleableLayer,
    pub show_parcels: ToggleableLayer,
    pub show_extra_shapes: ToggleableLayer,
    pub show_all_turn_icons: ToggleableLayer,
    pub debug_mode: ToggleableLayer,
}

impl ToggleableLayers {
    fn new() -> ToggleableLayers {
        ToggleableLayers {
            show_lanes: ToggleableLayer::new(
                DEBUG_LAYERS,
                "lanes",
                Key::D3,
                Some(MIN_ZOOM_FOR_LANES),
            ),
            show_buildings: ToggleableLayer::new(DEBUG_LAYERS, "buildings", Key::D1, Some(0.0)),
            show_intersections: ToggleableLayer::new(
                DEBUG_LAYERS,
                "intersections",
                Key::D2,
                Some(MIN_ZOOM_FOR_LANES),
            ),
            show_parcels: ToggleableLayer::new(
                DEBUG_LAYERS,
                "parcels",
                Key::D4,
                Some(MIN_ZOOM_FOR_PARCELS),
            ),
            show_extra_shapes: ToggleableLayer::new(
                DEBUG_LAYERS,
                "extra KML shapes",
                Key::D7,
                Some(MIN_ZOOM_FOR_LANES),
            ),
            show_all_turn_icons: ToggleableLayer::new(DEBUG_LAYERS, "turn icons", Key::D9, None),
            debug_mode: ToggleableLayer::new(DEBUG_LAYERS, "debug mode", Key::G, None),
        }
    }

    pub fn show(&self, id: ID) -> bool {
        match id {
            ID::Lane(_) => self.show_lanes.is_enabled(),
            ID::Building(_) => self.show_buildings.is_enabled(),
            ID::Intersection(_) => self.show_intersections.is_enabled(),
            ID::Parcel(_) => self.show_parcels.is_enabled(),
            ID::ExtraShape(_) => self.show_extra_shapes.is_enabled(),
            _ => true,
        }
    }
}

pub trait ShowTurnIcons {
    fn show_icons_for(&self, id: IntersectionID) -> bool;
}

impl ShowTurnIcons for UI {
    fn show_icons_for(&self, id: IntersectionID) -> bool {
        self.layers.show_all_turn_icons.is_enabled()
            || self.primary.stop_sign_editor.show_turn_icons(id)
            || self.primary.traffic_signal_editor.show_turn_icons(id)
    }
}

struct PluginCtx<'a> {
    ui: &'a mut UI,
    input: &'a mut UserInput,
    osd: &'a mut Text,
}
