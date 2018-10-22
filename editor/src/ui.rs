// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

// TODO this should just be a way to handle interactions between plugins

use abstutil;
use colors::{ColorScheme, Colors};
use control::ControlMap;
//use cpuprofiler;
use ezgui::{Canvas, Color, EventLoopMode, GfxCtx, Text, UserInput, BOTTOM_LEFT, GUI};
use flame;
use kml;
use map_model::{IntersectionID, Map};
use objects::{Ctx, ID, ROOT_MENU};
use piston::input::Key;
use plugins::a_b_tests::ABTestManager;
use plugins::chokepoints::ChokepointsFinder;
use plugins::classification::OsmClassifier;
use plugins::color_picker::ColorPicker;
use plugins::debug_objects::DebugObjectsState;
use plugins::diff_worlds::DiffWorldsState;
use plugins::draw_neighborhoods::DrawNeighborhoodState;
use plugins::floodfill::Floodfiller;
use plugins::follow::FollowState;
use plugins::geom_validation::Validator;
use plugins::hider::Hider;
use plugins::layers::ToggleableLayers;
use plugins::logs::DisplayLogs;
use plugins::map_edits::EditsManager;
use plugins::road_editor::RoadEditor;
use plugins::scenarios::ScenarioManager;
use plugins::search::SearchState;
use plugins::show_owner::ShowOwnerState;
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

const MIN_ZOOM_FOR_MOUSEOVER: f64 = 4.0;

pub struct UI {
    primary: PerMapUI,
    // When running an A/B test, this is populated too.
    secondary: Option<PerMapUI>,

    plugins: PluginsPerUI,

    // An index into plugin_handlers.
    active_plugin: Option<usize>,

    canvas: Canvas,
    cs: ColorScheme,

    // Remember this to support loading a new PerMapUI
    kml: Option<String>,

    plugin_handlers: Vec<Box<Fn(PluginCtx) -> bool>>,
}

impl GUI for UI {
    fn event(&mut self, mut input: UserInput, osd: &mut Text) -> EventLoopMode {
        // First update the camera and handle zoom
        let old_zoom = self.canvas.cam_zoom;
        self.canvas.handle_event(&mut input);
        let new_zoom = self.canvas.cam_zoom;
        self.plugins.layers.handle_zoom(old_zoom, new_zoom);

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
            if !self.plugin_handlers[idx](PluginCtx {
                primary: &mut self.primary,
                secondary: &mut self.secondary,
                plugins: &mut self.plugins,
                canvas: &mut self.canvas,
                cs: &mut self.cs,
                input: &mut input,
                osd,
                kml: &self.kml,
            }) {
                self.active_plugin = None;
            }
        } else {
            // Run each plugin, short-circuiting if the plugin claimed it was active.
            for (idx, plugin) in self.plugin_handlers.iter().enumerate() {
                if plugin(PluginCtx {
                    primary: &mut self.primary,
                    secondary: &mut self.secondary,
                    plugins: &mut self.plugins,
                    canvas: &mut self.canvas,
                    cs: &mut self.cs,
                    input: &mut input,
                    osd,
                    kml: &self.kml,
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
            //cpuprofiler::PROFILER.lock().unwrap().stop().unwrap();
            process::exit(0);
        }

        // Sim controller plugin is kind of always active? If nothing else ran, let it use keys.
        let result =
            self.plugins
                .sim_ctrl
                .event(&mut input, &mut self.primary, &mut self.secondary, osd);

        if self.primary.recalculate_current_selection {
            self.primary.recalculate_current_selection = false;
            self.primary.current_selection = self.mouseover_something();
        }

        input.populate_osd(osd);
        result
    }

    fn get_mut_canvas(&mut self) -> &mut Canvas {
        &mut self.canvas
    }

    fn draw(&self, g: &mut GfxCtx, osd: Text) {
        g.clear(self.cs.get(Colors::Background));

        let (statics, dynamics) = self.primary.draw_map.get_objects_onscreen(
            self.canvas.get_screen_bbox(),
            &self.primary.hider,
            &self.primary.map,
            &self.primary.sim,
            &self.plugins.layers,
            self,
        );
        for obj in statics.into_iter() {
            let opts = RenderOptions {
                color: self.color_obj(obj.get_id()),
                cam_zoom: self.canvas.cam_zoom,
                debug_mode: self.plugins.layers.debug_mode.is_enabled(),
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
                debug_mode: self.plugins.layers.debug_mode.is_enabled(),
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
        self.plugins.color_picker.draw(&self.canvas, g);
        self.primary.draw_neighborhoods.draw(g, &self.canvas);
        self.primary.scenarios.draw(g, &self.canvas);
        self.primary.edits_manager.draw(g, &self.canvas);
        self.plugins.ab_test_manager.draw(g, &self.canvas);
        self.plugins.logs.draw(g, &self.canvas);
        self.plugins.search_state.draw(g, &self.canvas);
        self.plugins.warp.draw(g, &self.canvas);
        self.plugins.sim_ctrl.draw(g, &self.canvas);
        self.primary.show_route.draw(g, &self.cs);
        self.plugins.diff_worlds.draw(g, &self.cs);

        self.canvas.draw_text(g, osd, BOTTOM_LEFT);
    }
}

// All of the state that's bound to a specific map+edit has to live here.
// TODO How can we arrange the code so that we statically know that we don't pass anything from UI
// to something in PerMapUI?
pub struct PerMapUI {
    pub map: Map,
    draw_map: DrawMap,
    pub control_map: ControlMap,
    pub sim: Sim,

    pub current_selection: Option<ID>,
    pub recalculate_current_selection: bool,
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
    show_owner: ShowOwnerState,
    draw_neighborhoods: DrawNeighborhoodState,
    scenarios: ScenarioManager,
    edits_manager: EditsManager,
    chokepoints: ChokepointsFinder,
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
            recalculate_current_selection: false,
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
            show_owner: ShowOwnerState::new(),
            draw_neighborhoods: DrawNeighborhoodState::new(),
            scenarios: ScenarioManager::new(),
            edits_manager: EditsManager::new(),
            chokepoints: ChokepointsFinder::new(),
        }
    }
}

// aka plugins that don't depend on map
struct PluginsPerUI {
    layers: ToggleableLayers,
    search_state: SearchState,
    warp: WarpState,
    osm_classifier: OsmClassifier,
    sim_ctrl: SimController,
    color_picker: ColorPicker,
    ab_test_manager: ABTestManager,
    logs: DisplayLogs,
    diff_worlds: DiffWorldsState,
}

impl UI {
    pub fn new(flags: SimFlags, kml: Option<String>) -> UI {
        // Do this first, so anything logged by sim::load isn't lost.
        let logs = DisplayLogs::new();

        let mut ui = UI {
            primary: PerMapUI::new(flags, &kml),
            secondary: None,

            plugins: PluginsPerUI {
                layers: ToggleableLayers::new(),
                search_state: SearchState::Empty,
                warp: WarpState::Empty,
                osm_classifier: OsmClassifier::new(),
                sim_ctrl: SimController::new(),
                color_picker: ColorPicker::new(),
                ab_test_manager: ABTestManager::new(),
                logs,
                diff_worlds: DiffWorldsState::new(),
            },

            active_plugin: None,

            canvas: Canvas::new(),
            cs: ColorScheme::load("color_scheme").unwrap(),

            kml,

            plugin_handlers: vec![
                Box::new(|ctx| {
                    if ctx.plugins.layers.event(ctx.input) {
                        ctx.primary.recalculate_current_selection = true;
                        true
                    } else {
                        false
                    }
                }),
                Box::new(|ctx| {
                    ctx.primary.traffic_signal_editor.event(
                        ctx.input,
                        &ctx.primary.map,
                        &mut ctx.primary.control_map,
                        ctx.primary.current_selection,
                    )
                }),
                Box::new(|ctx| {
                    ctx.primary.stop_sign_editor.event(
                        ctx.input,
                        &ctx.primary.map,
                        &mut ctx.primary.control_map,
                        ctx.primary.current_selection,
                    )
                }),
                Box::new(|ctx| {
                    ctx.primary.road_editor.event(
                        ctx.input,
                        ctx.primary.current_selection,
                        &mut ctx.primary.map,
                        &mut ctx.primary.draw_map,
                        &ctx.primary.control_map,
                        &mut ctx.primary.sim,
                    )
                }),
                Box::new(|ctx| ctx.plugins.search_state.event(ctx.input)),
                Box::new(|ctx| {
                    ctx.plugins.warp.event(
                        ctx.input,
                        &ctx.primary.map,
                        &ctx.primary.sim,
                        &ctx.primary.draw_map,
                        ctx.canvas,
                        &mut ctx.primary.current_selection,
                    )
                }),
                Box::new(|ctx| {
                    ctx.primary.follow.event(
                        ctx.input,
                        &ctx.primary.map,
                        &ctx.primary.sim,
                        ctx.canvas,
                        ctx.primary.current_selection,
                    )
                }),
                Box::new(|ctx| {
                    ctx.primary.show_route.event(
                        ctx.input,
                        &ctx.primary.sim,
                        &ctx.primary.map,
                        ctx.primary.current_selection,
                    )
                }),
                Box::new(|ctx| {
                    ctx.plugins
                        .color_picker
                        .event(ctx.input, ctx.canvas, ctx.cs)
                }),
                Box::new(|ctx| ctx.primary.steepness_viz.event(ctx.input)),
                Box::new(|ctx| ctx.plugins.osm_classifier.event(ctx.input)),
                Box::new(|ctx| {
                    ctx.primary
                        .hider
                        .event(ctx.input, &mut ctx.primary.current_selection)
                }),
                Box::new(|ctx| {
                    ctx.primary.debug_objects.event(
                        ctx.primary.current_selection,
                        ctx.input,
                        &ctx.primary.map,
                        &mut ctx.primary.sim,
                        &ctx.primary.control_map,
                    )
                }),
                Box::new(|ctx| {
                    ctx.primary.floodfiller.event(
                        &ctx.primary.map,
                        ctx.input,
                        ctx.primary.current_selection,
                    )
                }),
                Box::new(|ctx| {
                    ctx.primary.geom_validator.event(
                        ctx.input,
                        ctx.canvas,
                        &ctx.primary.map,
                        &ctx.primary.sim,
                        &ctx.primary.draw_map,
                    )
                }),
                Box::new(|ctx| {
                    ctx.primary
                        .turn_cycler
                        .event(ctx.input, ctx.primary.current_selection)
                }),
                Box::new(|ctx| {
                    ctx.primary.draw_neighborhoods.event(
                        ctx.input,
                        ctx.canvas,
                        &ctx.primary.map,
                        ctx.osd,
                    )
                }),
                Box::new(|ctx| {
                    ctx.primary
                        .scenarios
                        .event(ctx.input, &ctx.primary.map, &mut ctx.primary.sim)
                }),
                Box::new(|ctx| {
                    let (active, new_primary) = ctx.primary.edits_manager.event(
                        ctx.input,
                        &ctx.primary.map,
                        &ctx.primary.control_map,
                        &ctx.primary.road_editor,
                        &mut ctx.primary.current_flags,
                        ctx.kml,
                    );
                    if new_primary.is_some() {
                        *ctx.primary = new_primary.unwrap();
                    }
                    active
                }),
                Box::new(|ctx| {
                    ctx.primary
                        .chokepoints
                        .event(ctx.input, &ctx.primary.sim, &ctx.primary.map)
                }),
                Box::new(|ctx| {
                    let (active, new_ui) = ctx.plugins.ab_test_manager.event(
                        ctx.input,
                        ctx.primary.current_selection,
                        &ctx.primary.map,
                        ctx.kml,
                        &ctx.primary.current_flags,
                    );
                    if let Some((new_primary, new_secondary)) = new_ui {
                        *ctx.primary = new_primary;
                        *ctx.secondary = Some(new_secondary);
                    }
                    active
                }),
                Box::new(|ctx| ctx.plugins.logs.event(ctx.input)),
                Box::new(|ctx| {
                    ctx.plugins
                        .diff_worlds
                        .event(ctx.input, &ctx.primary, ctx.secondary)
                }),
                Box::new(|ctx| {
                    ctx.primary
                        .show_owner
                        .event(ctx.primary.current_selection, &ctx.primary.sim);
                    // TODO This is a weird exception -- this plugin doesn't consume input, so
                    // never treat it as active for blocking input
                    false
                }),
            ],
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

        ui.plugins.layers.handle_zoom(-1.0, ui.canvas.cam_zoom);

        ui
    }

    fn mouseover_something(&self) -> Option<ID> {
        let pt = self.canvas.get_cursor_in_map_space();

        let (statics, dynamics) = self.primary.draw_map.get_objects_onscreen(
            self.canvas.get_screen_bbox(),
            &self.primary.hider,
            &self.primary.map,
            &self.primary.sim,
            &self.plugins.layers,
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

    fn color_obj(&self, id: ID) -> Option<Color> {
        if Some(id) == self.primary.current_selection {
            return Some(self.cs.get(Colors::Selected));
        }

        let ctx = Ctx {
            cs: &self.cs,
            map: &self.primary.map,
            control_map: &self.primary.control_map,
            canvas: &self.canvas,
            sim: &self.primary.sim,
        };
        if let Some(p) = self.get_active_plugin() {
            return p.color_for(id, ctx);
        }

        // TODO Ew, this is a weird ambient plugin that doesn't consume input but has an opinion on
        // color.
        self.primary.show_owner.color_for(id, ctx)
    }

    fn get_active_plugin(&self) -> Option<Box<&Colorizer>> {
        let idx = self.active_plugin?;
        // Match instead of array, because can't move the Box out of the temporary vec. :\
        // This must line up with the list of plugins in UI::new.
        match idx {
            0 => Some(Box::new(&self.plugins.layers)),
            1 => Some(Box::new(&self.primary.traffic_signal_editor)),
            2 => Some(Box::new(&self.primary.stop_sign_editor)),
            3 => Some(Box::new(&self.primary.road_editor)),
            4 => Some(Box::new(&self.plugins.search_state)),
            5 => Some(Box::new(&self.plugins.warp)),
            6 => Some(Box::new(&self.primary.follow)),
            7 => Some(Box::new(&self.primary.show_route)),
            8 => Some(Box::new(&self.plugins.color_picker)),
            9 => Some(Box::new(&self.primary.steepness_viz)),
            10 => Some(Box::new(&self.plugins.osm_classifier)),
            11 => Some(Box::new(&self.primary.hider)),
            12 => Some(Box::new(&self.primary.debug_objects)),
            13 => Some(Box::new(&self.primary.floodfiller)),
            14 => Some(Box::new(&self.primary.geom_validator)),
            15 => Some(Box::new(&self.primary.turn_cycler)),
            16 => Some(Box::new(&self.primary.draw_neighborhoods)),
            17 => Some(Box::new(&self.primary.scenarios)),
            18 => Some(Box::new(&self.primary.edits_manager)),
            19 => Some(Box::new(&self.primary.chokepoints)),
            20 => Some(Box::new(&self.plugins.ab_test_manager)),
            21 => Some(Box::new(&self.plugins.logs)),
            22 => Some(Box::new(&self.plugins.diff_worlds)),
            23 => Some(Box::new(&self.primary.show_owner)),
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

pub trait ShowTurnIcons {
    fn show_icons_for(&self, id: IntersectionID) -> bool;
}

impl ShowTurnIcons for UI {
    fn show_icons_for(&self, id: IntersectionID) -> bool {
        self.plugins.layers.show_all_turn_icons.is_enabled()
            || self.primary.stop_sign_editor.show_turn_icons(id)
            || self.primary.traffic_signal_editor.show_turn_icons(id)
    }
}

// TODO I can't help but noticing this is just UI but with references. Can we be more direct?
struct PluginCtx<'a> {
    primary: &'a mut PerMapUI,
    secondary: &'a mut Option<PerMapUI>,
    plugins: &'a mut PluginsPerUI,
    canvas: &'a mut Canvas,
    cs: &'a mut ColorScheme,
    input: &'a mut UserInput,
    osd: &'a mut Text,
    kml: &'a Option<String>,
}
