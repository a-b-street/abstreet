// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

// TODO this should just be a way to handle interactions between plugins

use abstutil;
use colors::{ColorScheme, Colors};
use control::ControlMap;
use control::{ModifiedStopSign, ModifiedTrafficSignal};
use ezgui::{Canvas, EventLoopMode, GfxCtx, TextOSD, ToggleableLayer, UserInput, GUI};
use flame;
use graphics::types::Color;
use kml;
use map_model;
use map_model::IntersectionID;
use objects::{Ctx, DEBUG_LAYERS, ID, ROOT_MENU};
use piston::input::{Key, MouseCursorEvent};
use piston::window::Size;
use plugins::classification::OsmClassifier;
use plugins::color_picker::ColorPicker;
use plugins::debug_objects::DebugObjectsState;
use plugins::draw_polygon::DrawPolygonState;
use plugins::floodfill::Floodfiller;
use plugins::follow::FollowState;
use plugins::geom_validation::Validator;
use plugins::hider::Hider;
use plugins::logs::DisplayLogs;
use plugins::road_editor::RoadEditor;
use plugins::search::SearchState;
use plugins::show_route::ShowRouteState;
use plugins::sim_controls::SimController;
use plugins::steep::SteepnessVisualizer;
use plugins::stop_sign_editor::StopSignEditor;
use plugins::traffic_signal_editor::TrafficSignalEditor;
use plugins::turn_cycler::TurnCyclerState;
use plugins::warp::WarpState;
use plugins::wizard::WizardSample;
use plugins::Colorizer;
use render::{DrawMap, RenderOptions};
use sim;
use sim::Sim;
use std::collections::HashMap;
use std::process;

// TODO ideally these would be tuned kind of dynamically based on rendering speed
const MIN_ZOOM_FOR_LANES: f64 = 0.15;
const MIN_ZOOM_FOR_PARCELS: f64 = 1.0;
const MIN_ZOOM_FOR_MOUSEOVER: f64 = 4.0;

// Necessary so we can iterate over and run the plugins, which mutably borrow UI.
pub struct UIWrapper {
    ui: UI,
    plugins: Vec<Box<Fn(&mut UI, &mut UserInput, &mut TextOSD) -> bool>>,
}

impl GUI for UIWrapper {
    fn event(&mut self, input: UserInput, osd: &mut TextOSD) -> EventLoopMode {
        self.ui.event(input, osd, &self.plugins)
    }

    fn draw(&mut self, g: &mut GfxCtx, osd: TextOSD, window_size: Size) {
        // Since self is mut here, we can set window_size on the canvas, but then let the real
        // draw() be immutable.
        self.ui.canvas.start_drawing(g, window_size);
        self.ui.draw(g, osd);
    }
}

impl UIWrapper {
    // nit: lots of this logic could live in UI, if it mattered
    pub fn new(
        load: String,
        scenario_name: String,
        rng_seed: Option<u8>,
        kml: Option<String>,
    ) -> UIWrapper {
        flame::start("setup");
        let (map, edits, control_map, sim) = sim::load(
            load,
            scenario_name,
            rng_seed,
            Some(sim::Tick::from_seconds(30)),
        );

        let extra_shapes = if let Some(path) = kml {
            kml::load(&path, &map.get_gps_bounds()).expect("Couldn't load extra KML shapes")
        } else {
            Vec::new()
        };

        flame::start("draw_map");
        let (draw_map, center_pt) = DrawMap::new(&map, &control_map, extra_shapes);
        flame::end("draw_map");

        flame::end("setup");
        flame::dump_stdout();

        let steepness_viz = SteepnessVisualizer::new(&map);

        let mut ui = UI {
            // TODO organize this by section
            map,
            draw_map,
            control_map,
            sim,

            steepness_viz,
            sim_ctrl: SimController::new(),

            layers: ToggleableLayers::new(),

            current_selection: None,

            hider: Hider::new(),
            debug_objects: DebugObjectsState::new(),
            search_state: SearchState::Empty,
            warp: WarpState::Empty,
            follow: FollowState::Empty,
            show_route: ShowRouteState::Empty,
            floodfiller: Floodfiller::new(),
            osm_classifier: OsmClassifier::new(),
            traffic_signal_editor: TrafficSignalEditor::new(),
            stop_sign_editor: StopSignEditor::new(),
            road_editor: RoadEditor::new(edits),
            color_picker: ColorPicker::new(),
            geom_validator: Validator::new(),
            turn_cycler: TurnCyclerState::new(),
            draw_polygon: DrawPolygonState::new(),
            wizard_sample: WizardSample::new(),
            logs: DisplayLogs::new(),

            active_plugin: None,

            canvas: Canvas::new(),
            cs: ColorScheme::load("color_scheme").unwrap(),
        };

        match abstutil::read_json::<EditorState>("editor_state") {
            Ok(ref state) if *ui.map.get_name() == state.map_name => {
                info!(target: "UI", "Loaded previous editor_state");
                ui.canvas.cam_x = state.cam_x;
                ui.canvas.cam_y = state.cam_y;
                ui.canvas.cam_zoom = state.cam_zoom;
                ui.control_map
                    .load_savestate(&state.traffic_signals, &state.stop_signs);
            }
            _ => {
                warn!(target: "UI", "Couldn't load editor_state or it's for a different map, so just centering initial view");
                ui.canvas.center_on_map_pt(center_pt);
            }
        }

        let new_zoom = ui.canvas.cam_zoom;
        for layer in ui.toggleable_layers().into_iter() {
            layer.handle_zoom(-1.0, new_zoom);
        }

        UIWrapper {
            ui,
            plugins: vec![
                Box::new(|ui, input, _osd| {
                    let layer_changed = {
                        let mut changed = false;
                        for layer in ui.toggleable_layers().into_iter() {
                            if layer.event(input) {
                                changed = true;
                                break;
                            }
                        }
                        changed
                    };
                    if layer_changed {
                        ui.current_selection = ui.mouseover_something();
                        true
                    } else {
                        false
                    }
                }),
                Box::new(|ui, input, _osd| {
                    ui.traffic_signal_editor.event(
                        input,
                        &ui.map,
                        &mut ui.control_map,
                        ui.current_selection,
                    )
                }),
                Box::new(|ui, input, _osd| {
                    ui.stop_sign_editor.event(
                        input,
                        &ui.map,
                        &mut ui.control_map,
                        ui.current_selection,
                    )
                }),
                Box::new(|ui, input, _osd| {
                    ui.road_editor.event(
                        input,
                        ui.current_selection,
                        &mut ui.map,
                        &mut ui.draw_map,
                        &ui.control_map,
                        &mut ui.sim,
                    )
                }),
                Box::new(|ui, input, _osd| ui.search_state.event(input)),
                Box::new(|ui, input, _osd| {
                    ui.warp.event(
                        input,
                        &ui.map,
                        &ui.sim,
                        &mut ui.canvas,
                        &mut ui.current_selection,
                    )
                }),
                Box::new(|ui, input, _osd| {
                    ui.follow.event(
                        input,
                        &ui.map,
                        &ui.sim,
                        &mut ui.canvas,
                        ui.current_selection,
                    )
                }),
                Box::new(|ui, input, _osd| {
                    ui.show_route.event(input, &ui.sim, ui.current_selection)
                }),
                Box::new(|ui, input, _osd| {
                    ui.color_picker.event(input, &mut ui.canvas, &mut ui.cs)
                }),
                Box::new(|ui, input, _osd| ui.steepness_viz.event(input)),
                Box::new(|ui, input, _osd| ui.osm_classifier.event(input)),
                Box::new(|ui, input, _osd| ui.hider.event(input, &mut ui.current_selection)),
                Box::new(|ui, input, _osd| {
                    ui.debug_objects.event(
                        ui.current_selection,
                        input,
                        &ui.map,
                        &mut ui.sim,
                        &ui.control_map,
                    )
                }),
                Box::new(|ui, input, _osd| {
                    ui.floodfiller.event(&ui.map, input, ui.current_selection)
                }),
                Box::new(|ui, input, _osd| {
                    ui.geom_validator
                        .event(input, &mut ui.canvas, &ui.map, &ui.draw_map)
                }),
                Box::new(|ui, input, _osd| ui.turn_cycler.event(input, ui.current_selection)),
                Box::new(|ui, input, osd| ui.draw_polygon.event(input, &ui.canvas, &ui.map, osd)),
                Box::new(|ui, input, _osd| ui.wizard_sample.event(input, &ui.map)),
                Box::new(|ui, input, _osd| ui.logs.event(input)),
            ],
        }
    }
}

struct UI {
    map: map_model::Map,
    draw_map: DrawMap,
    control_map: ControlMap,
    sim: Sim,

    layers: ToggleableLayers,

    current_selection: Option<ID>,

    hider: Hider,
    debug_objects: DebugObjectsState,
    search_state: SearchState,
    warp: WarpState,
    follow: FollowState,
    show_route: ShowRouteState,
    floodfiller: Floodfiller,
    steepness_viz: SteepnessVisualizer,
    osm_classifier: OsmClassifier,
    traffic_signal_editor: TrafficSignalEditor,
    stop_sign_editor: StopSignEditor,
    road_editor: RoadEditor,
    sim_ctrl: SimController,
    color_picker: ColorPicker,
    geom_validator: Validator,
    turn_cycler: TurnCyclerState,
    draw_polygon: DrawPolygonState,
    wizard_sample: WizardSample,
    logs: DisplayLogs,

    // An index into UIWrapper.plugins.
    active_plugin: Option<usize>,

    canvas: Canvas,
    cs: ColorScheme,
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

        let (statics, dynamics) = self.draw_map.get_objects_onscreen(
            self.canvas.get_screen_bbox(),
            &self.hider,
            &self.map,
            &self.sim,
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
        osd: &mut TextOSD,
        plugins: &Vec<Box<Fn(&mut UI, &mut UserInput, &mut TextOSD) -> bool>>,
    ) -> EventLoopMode {
        // First update the camera and handle zoom
        let old_zoom = self.canvas.cam_zoom;
        self.canvas.handle_event(input.use_event_directly());
        let new_zoom = self.canvas.cam_zoom;
        for layer in self.toggleable_layers().into_iter() {
            layer.handle_zoom(old_zoom, new_zoom);
        }

        // Always handle mouseover
        if old_zoom >= MIN_ZOOM_FOR_MOUSEOVER && new_zoom < MIN_ZOOM_FOR_MOUSEOVER {
            self.current_selection = None;
        }
        if !self.canvas.is_dragging()
            && input.use_event_directly().mouse_cursor_args().is_some()
            && new_zoom >= MIN_ZOOM_FOR_MOUSEOVER
        {
            self.current_selection = self.mouseover_something();
        }

        // TODO Normally we'd return InputOnly here if there was an active plugin, but actually, we
        // want some keys to always be pressable (sim controller stuff, quitting the game?)

        // If there's an active plugin, just run it.
        if let Some(idx) = self.active_plugin {
            if !plugins[idx](self, &mut input, osd) {
                self.active_plugin = None;
            }
        } else {
            // Run each plugin, short-circuiting if the plugin claimed it was active.
            for (idx, plugin) in plugins.iter().enumerate() {
                if plugin(self, &mut input, osd) {
                    self.active_plugin = Some(idx);
                    break;
                }
            }
        }

        if input.unimportant_key_pressed(Key::Escape, ROOT_MENU, "quit") {
            let state = EditorState {
                map_name: self.map.get_name().clone(),
                cam_x: self.canvas.cam_x,
                cam_y: self.canvas.cam_y,
                cam_zoom: self.canvas.cam_zoom,
                traffic_signals: self.control_map.get_traffic_signals_savestate(),
                stop_signs: self.control_map.get_stop_signs_savestate(),
            };
            // TODO maybe make state line up with the map, so loading from a new map doesn't break
            abstutil::write_json("editor_state", &state).expect("Saving editor_state failed");
            abstutil::write_json("color_scheme", &self.cs).expect("Saving color_scheme failed");
            abstutil::write_json("road_edits.json", self.road_editor.get_edits())
                .expect("Saving road_edits.json failed");
            println!("Saved editor_state, color_scheme, and road_edits.json");
            process::exit(0);
        }

        // Sim controller plugin is kind of always active? If nothing else ran, let it use keys.
        let result = self.sim_ctrl.event(
            &mut input,
            &self.map,
            &self.control_map,
            &mut self.sim,
            self.current_selection,
            osd,
        );
        input.populate_osd(osd);
        result
    }

    fn draw(&self, g: &mut GfxCtx, osd: TextOSD) {
        g.clear(self.cs.get(Colors::Background));

        let (statics, dynamics) = self.draw_map.get_objects_onscreen(
            self.canvas.get_screen_bbox(),
            &self.hider,
            &self.map,
            &self.sim,
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
                    map: &self.map,
                    control_map: &self.control_map,
                    canvas: &self.canvas,
                    sim: &self.sim,
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
                    map: &self.map,
                    control_map: &self.control_map,
                    canvas: &self.canvas,
                    sim: &self.sim,
                },
            );
        }

        // TODO Only if active?
        self.turn_cycler.draw(
            &self.map,
            &self.draw_map,
            &self.control_map,
            &self.sim,
            &self.cs,
            g,
        );
        self.debug_objects
            .draw(&self.map, &self.canvas, &self.draw_map, &self.sim, g);
        self.color_picker.draw(&self.canvas, g);
        self.draw_polygon.draw(g, &self.canvas);
        self.wizard_sample.draw(g, &self.canvas);
        self.logs.draw(g, &self.canvas);
        self.search_state.draw(g, &self.canvas);
        self.warp.draw(g, &self.canvas);

        self.canvas.draw_osd_notification(g, osd);
    }

    fn color_obj(&self, id: ID) -> Option<Color> {
        if Some(id) == self.current_selection {
            return Some(self.cs.get(Colors::Selected));
        }

        if let Some(p) = self.get_active_plugin() {
            if let Some(c) = p.color_for(
                id,
                Ctx {
                    cs: &self.cs,
                    map: &self.map,
                    control_map: &self.control_map,
                    canvas: &self.canvas,
                    sim: &self.sim,
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
            1 => Some(Box::new(&self.traffic_signal_editor)),
            2 => Some(Box::new(&self.stop_sign_editor)),
            3 => Some(Box::new(&self.road_editor)),
            4 => Some(Box::new(&self.search_state)),
            5 => Some(Box::new(&self.warp)),
            6 => Some(Box::new(&self.follow)),
            7 => Some(Box::new(&self.show_route)),
            8 => Some(Box::new(&self.color_picker)),
            9 => Some(Box::new(&self.steepness_viz)),
            10 => Some(Box::new(&self.osm_classifier)),
            11 => Some(Box::new(&self.hider)),
            12 => Some(Box::new(&self.debug_objects)),
            13 => Some(Box::new(&self.floodfiller)),
            14 => Some(Box::new(&self.geom_validator)),
            15 => Some(Box::new(&self.turn_cycler)),
            16 => Some(Box::new(&self.draw_polygon)),
            17 => Some(Box::new(&self.wizard_sample)),
            18 => Some(Box::new(&self.logs)),
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

    pub traffic_signals: HashMap<IntersectionID, ModifiedTrafficSignal>,
    pub stop_signs: HashMap<IntersectionID, ModifiedStopSign>,
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
            || self.stop_sign_editor.show_turn_icons(id)
            || self.traffic_signal_editor.show_turn_icons(id)
    }
}
