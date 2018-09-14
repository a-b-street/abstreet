// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

// TODO this should just be a way to handle interactions between plugins

use abstutil;
use colors::{ColorScheme, Colors};
use control::ControlMap;
use control::{ModifiedStopSign, ModifiedTrafficSignal};
use ezgui::{shift_color, Canvas, EventLoopMode, GfxCtx, ToggleableLayer, UserInput, GUI};
use flame;
use geom::Pt2D;
use graphics::types::Color;
use kml;
use map_model;
use map_model::IntersectionID;
use objects::ID;
use piston::input::{Key, MouseCursorEvent};
use piston::window::Size;
use plugins::classification::OsmClassifier;
use plugins::color_picker::ColorPicker;
use plugins::debug_objects::DebugObjectsState;
use plugins::floodfill::Floodfiller;
use plugins::follow::FollowState;
use plugins::geom_validation::Validator;
use plugins::hider::Hider;
use plugins::road_editor::RoadEditor;
use plugins::search::SearchState;
use plugins::show_route::ShowRouteState;
use plugins::sim_controls::SimController;
use plugins::steep::SteepnessVisualizer;
use plugins::stop_sign_editor::StopSignEditor;
use plugins::traffic_signal_editor::TrafficSignalEditor;
use plugins::turn_colors::TurnColors;
use plugins::turn_cycler::TurnCyclerState;
use plugins::warp::WarpState;
use render::{DrawMap, RenderOptions};
use sim;
use sim::{CarID, CarState, PedestrianID, Sim};
use std::collections::HashMap;
use std::process;

// TODO ideally these would be tuned kind of dynamically based on rendering speed
const MIN_ZOOM_FOR_LANES: f64 = 0.15;
const MIN_ZOOM_FOR_PARCELS: f64 = 1.0;
const MIN_ZOOM_FOR_MOUSEOVER: f64 = 1.0;

// Necessary so we can iterate over and run the plugins, which mutably borrow UI.
pub struct UIWrapper {
    ui: UI,
    plugins: Vec<Box<Fn(&mut UI, &mut UserInput) -> bool>>,
}

impl GUI for UIWrapper {
    fn event(&mut self, input: &mut UserInput) -> EventLoopMode {
        self.ui.event(input, &self.plugins)
    }

    fn draw(&mut self, g: &mut GfxCtx, input: UserInput, window_size: Size) {
        // Since self is mut here, we can set window_size on the canvas, but then let the real
        // draw() be immutable.
        self.ui.canvas.start_drawing(g, window_size);
        self.ui.draw(g, input);
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
        let turn_colors = TurnColors::new(&control_map);

        let mut ui = UI {
            // TODO organize this by section
            map,
            draw_map,
            control_map,
            sim,

            steepness_viz,
            turn_colors,
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

            active_plugin: None,

            canvas: Canvas::new(),
            cs: ColorScheme::load("color_scheme").unwrap(),
        };

        match abstutil::read_json::<EditorState>("editor_state") {
            Ok(ref state) if *ui.map.get_name() == state.map_name => {
                println!("Loaded previous editor_state");
                ui.canvas.cam_x = state.cam_x;
                ui.canvas.cam_y = state.cam_y;
                ui.canvas.cam_zoom = state.cam_zoom;
                ui.control_map
                    .load_savestate(&state.traffic_signals, &state.stop_signs);
            }
            _ => {
                println!("Couldn't load editor_state or it's for a different map, so just centering initial view");
                ui.canvas.center_on_map_pt(center_pt.x(), center_pt.y());
            }
        }

        let new_zoom = ui.canvas.cam_zoom;
        for layer in ui.toggleable_layers().into_iter() {
            layer.handle_zoom(-1.0, new_zoom);
        }

        UIWrapper {
            ui,
            plugins: vec![
                Box::new(|ui, input| {
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
                Box::new(|ui, input| {
                    ui.traffic_signal_editor.event(
                        input,
                        &ui.map,
                        &mut ui.control_map,
                        ui.current_selection,
                    )
                }),
                Box::new(|ui, input| {
                    ui.stop_sign_editor.event(
                        input,
                        &ui.map,
                        &mut ui.control_map,
                        ui.current_selection,
                    )
                }),
                Box::new(|ui, input| {
                    ui.road_editor.event(
                        input,
                        ui.current_selection,
                        &mut ui.map,
                        &mut ui.draw_map,
                        &ui.control_map,
                        &mut ui.sim,
                    )
                }),
                Box::new(|ui, input| ui.search_state.event(input)),
                Box::new(|ui, input| {
                    ui.warp.event(
                        input,
                        &ui.map,
                        &ui.sim,
                        &mut ui.canvas,
                        &mut ui.current_selection,
                    )
                }),
                Box::new(|ui, input| {
                    ui.follow.event(
                        input,
                        &ui.map,
                        &ui.sim,
                        &mut ui.canvas,
                        ui.current_selection,
                    )
                }),
                Box::new(|ui, input| ui.show_route.event(input, &ui.sim, ui.current_selection)),
                Box::new(|ui, input| ui.color_picker.event(input, &mut ui.canvas, &mut ui.cs)),
                Box::new(|ui, input| ui.steepness_viz.event(input)),
                Box::new(|ui, input| ui.osm_classifier.event(input)),
                Box::new(|ui, input| ui.hider.event(input, &mut ui.current_selection)),
                Box::new(|ui, input| {
                    ui.debug_objects.event(
                        ui.current_selection,
                        input,
                        &ui.map,
                        &mut ui.sim,
                        &ui.control_map,
                    )
                }),
                Box::new(|ui, input| ui.floodfiller.event(&ui.map, input, ui.current_selection)),
                Box::new(|ui, input| {
                    ui.geom_validator
                        .event(input, &mut ui.canvas, &ui.map, &ui.draw_map)
                }),
                Box::new(|ui, input| ui.turn_cycler.event(input, ui.current_selection)),
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

    // An index into UIWrapper.plugins.
    active_plugin: Option<usize>,

    // Not really a plugin; it doesn't react to anything.
    turn_colors: TurnColors,

    canvas: Canvas,
    // TODO maybe never pass this to other places? Always resolve colors here?
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
        let (x, y) = self.canvas.get_cursor_in_map_space();
        let pt = Pt2D::new(x, y);

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

    fn color_lane(&self, id: map_model::LaneID) -> Color {
        let l = self.map.get_l(id);
        let mut default = match l.lane_type {
            map_model::LaneType::Driving => self.cs.get(Colors::Road),
            map_model::LaneType::Parking => self.cs.get(Colors::Parking),
            map_model::LaneType::Sidewalk => self.cs.get(Colors::Sidewalk),
            map_model::LaneType::Biking => self.cs.get(Colors::Biking),
        };
        if l.probably_broken {
            default = self.cs.get(Colors::Broken);
        }

        // TODO This evaluates all the color methods, which may be expensive. But the option
        // chaining is harder to read. :(
        vec![
            self.color_for_selected(ID::Lane(l.id)),
            self.show_route.color_l(l.id, &self.cs),
            self.search_state.color_l(l, &self.map, &self.cs),
            self.floodfiller.color_l(l, &self.cs),
            self.steepness_viz.color_l(&self.map, l),
            self.osm_classifier.color_l(l, &self.map, &self.cs),
        ].iter()
            .filter_map(|c| *c)
            .next()
            .unwrap_or(default)
    }

    fn color_intersection(&self, id: map_model::IntersectionID) -> Color {
        let i = self.map.get_i(id);
        let changed = if let Some(s) = self.control_map.traffic_signals.get(&i.id) {
            s.changed()
        } else if let Some(s) = self.control_map.stop_signs.get(&i.id) {
            s.changed()
        } else {
            false
        };
        let default_color = if changed {
            self.cs.get(Colors::ChangedIntersection)
        } else {
            self.cs.get(Colors::UnchangedIntersection)
        };

        self.color_for_selected(ID::Intersection(i.id))
            .unwrap_or(default_color)
    }

    fn color_turn_icon(&self, id: map_model::TurnID) -> Color {
        let t = self.map.get_t(id);
        // TODO traffic signal selection logic maybe moves here
        self.color_for_selected(ID::Turn(t.id)).unwrap_or_else(|| {
            self.stop_sign_editor
                .color_t(t, &self.control_map, &self.cs)
                .unwrap_or_else(|| {
                    self.traffic_signal_editor
                        .color_t(t, &self.map, &self.control_map, &self.cs)
                        .unwrap_or_else(|| {
                            self.turn_colors
                                .color_t(t)
                                .unwrap_or(self.cs.get(Colors::TurnIconInactive))
                        })
                })
        })
    }

    fn color_building(&self, id: map_model::BuildingID) -> Color {
        let b = self.map.get_b(id);
        vec![
            self.color_for_selected(ID::Building(b.id)),
            self.search_state.color_b(b, &self.cs),
            self.osm_classifier.color_b(b, &self.cs),
        ].iter()
            .filter_map(|c| *c)
            .next()
            .unwrap_or(self.cs.get(Colors::Building))
    }

    // Returns (boundary, fill) color
    fn color_parcel(&self, id: map_model::ParcelID) -> Color {
        const COLORS: [Color; 14] = [
            // TODO these are awful choices
            [1.0, 1.0, 0.0, 1.0],
            [1.0, 0.0, 1.0, 1.0],
            [0.0, 1.0, 1.0, 1.0],
            [0.5, 0.2, 0.7, 1.0],
            [0.5, 0.5, 0.0, 0.5],
            [0.5, 0.0, 0.5, 0.5],
            [0.0, 0.5, 0.5, 0.5],
            [0.0, 0.0, 0.5, 0.5],
            [0.3, 0.2, 0.5, 0.5],
            [0.4, 0.2, 0.5, 0.5],
            [0.5, 0.2, 0.5, 0.5],
            [0.6, 0.2, 0.5, 0.5],
            [0.7, 0.2, 0.5, 0.5],
            [0.8, 0.2, 0.5, 0.5],
        ];
        let p = self.map.get_p(id);
        /*(
            self.cs.get(Colors::ParcelBoundary),
            COLORS[p.block % COLORS.len()],
        )*/
        self.color_for_selected(ID::Parcel(p.id))
            .unwrap_or(COLORS[p.block % COLORS.len()])
    }

    fn color_car(&self, id: CarID) -> Color {
        if let Some(c) = self.color_for_selected(ID::Car(id)) {
            return c;
        }
        // TODO if it's a bus, color it differently -- but how? :\
        match self.sim.get_car_state(id) {
            CarState::Debug => shift_color(self.cs.get(Colors::DebugCar), id.0),
            CarState::Moving => shift_color(self.cs.get(Colors::MovingCar), id.0),
            CarState::Stuck => shift_color(self.cs.get(Colors::StuckCar), id.0),
            CarState::Parked => shift_color(self.cs.get(Colors::ParkedCar), id.0),
        }
    }

    fn color_ped(&self, id: PedestrianID) -> Color {
        if let Some(c) = self.color_for_selected(ID::Pedestrian(id)) {
            return c;
        }
        shift_color(self.cs.get(Colors::Pedestrian), id.0)
    }

    fn color_for_selected(&self, id: ID) -> Option<Color> {
        if Some(id) == self.current_selection {
            Some(self.cs.get(Colors::Selected))
        } else {
            None
        }
    }

    fn event(
        &mut self,
        input: &mut UserInput,
        plugins: &Vec<Box<Fn(&mut UI, &mut UserInput) -> bool>>,
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
            if !plugins[idx](self, input) {
                self.active_plugin = None;
            }
        } else {
            // Run each plugin, short-circuiting if the plugin claimed it was active.
            for (idx, plugin) in plugins.iter().enumerate() {
                if plugin(self, input) {
                    self.active_plugin = Some(idx);
                    break;
                }
            }
        }

        if input.unimportant_key_pressed(Key::Escape, "quit") {
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
        self.sim_ctrl.event(
            input,
            &self.map,
            &self.control_map,
            &mut self.sim,
            self.current_selection,
        )
    }

    fn draw(&self, g: &mut GfxCtx, input: UserInput) {
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
            let color = match obj.get_id() {
                ID::Parcel(id) => self.color_parcel(id),
                ID::Lane(id) => self.color_lane(id),
                ID::Intersection(id) => self.color_intersection(id),
                ID::Turn(id) => self.color_turn_icon(id),
                // TODO and self.cs.get(Colors::BuildingBoundary),
                ID::Building(id) => self.color_building(id),
                ID::ExtraShape(id) => self.color_for_selected(ID::ExtraShape(id))
                    .unwrap_or(self.cs.get(Colors::ExtraShape)),

                ID::Car(_) | ID::Pedestrian(_) => {
                    panic!("Dynamic {:?} in statics list", obj.get_id())
                }
            };
            let opts = RenderOptions {
                color,
                cam_zoom: self.canvas.cam_zoom,
                debug_mode: self.layers.debug_mode.is_enabled(),
            };
            obj.draw(g, opts, &self.cs);
        }
        for obj in dynamics.into_iter() {
            let color = match obj.get_id() {
                ID::Car(id) => self.color_car(id),
                ID::Pedestrian(id) => self.color_ped(id),
                _ => panic!("Static {:?} in dynamics list", obj.get_id()),
            };
            let opts = RenderOptions {
                color,
                cam_zoom: self.canvas.cam_zoom,
                debug_mode: self.layers.debug_mode.is_enabled(),
            };
            obj.draw(g, opts, &self.cs);
        }

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

        let mut osd_lines = self.sim_ctrl.get_osd_lines(&self.sim);
        let action_lines = input.get_possible_actions();
        if !action_lines.is_empty() {
            osd_lines.push(String::from(""));
            osd_lines.extend(action_lines);
        }
        let search_lines = self.search_state.get_osd_lines();
        if !search_lines.is_empty() {
            osd_lines.push(String::from(""));
            osd_lines.extend(search_lines);
        }
        let warp_lines = self.warp.get_osd_lines();
        if !warp_lines.is_empty() {
            osd_lines.push(String::from(""));
            osd_lines.extend(warp_lines);
        }
        self.canvas.draw_osd_notification(g, &osd_lines);
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
            show_lanes: ToggleableLayer::new("lanes", Key::D3, Some(MIN_ZOOM_FOR_LANES)),
            show_buildings: ToggleableLayer::new("buildings", Key::D1, Some(0.0)),
            show_intersections: ToggleableLayer::new(
                "intersections",
                Key::D2,
                Some(MIN_ZOOM_FOR_LANES),
            ),
            show_parcels: ToggleableLayer::new("parcels", Key::D4, Some(MIN_ZOOM_FOR_PARCELS)),
            show_extra_shapes: ToggleableLayer::new(
                "extra KML shapes",
                Key::D7,
                Some(MIN_ZOOM_FOR_LANES),
            ),
            show_all_turn_icons: ToggleableLayer::new("turn icons", Key::D9, None),
            debug_mode: ToggleableLayer::new("debug mode", Key::G, None),
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
