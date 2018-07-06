// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

// TODO this should just be a way to handle interactions between plugins

use abstutil;
use colors::{ColorScheme, Colors};
use control::ControlMap;
use control::{ModifiedStopSign, ModifiedTrafficSignal};
use ezgui::GfxCtx;
use ezgui::ToggleableLayer;
use ezgui::canvas::Canvas;
use ezgui::input::UserInput;
use graphics::types::Color;
use gui;
use map_model;
use map_model::IntersectionID;
use piston::input::{Key, MouseCursorEvent};
use piston::window::Size;
use plugins::classification::OsmClassifier;
use plugins::color_picker::ColorPicker;
use plugins::floodfill::Floodfiller;
use plugins::geom_validation::Validator;
use plugins::search::SearchState;
use plugins::selection::{Hider, SelectionState, ID};
use plugins::sim_controls::SimController;
use plugins::steep::SteepnessVisualizer;
use plugins::stop_sign_editor::StopSignEditor;
use plugins::traffic_signal_editor::TrafficSignalEditor;
use plugins::turn_colors::TurnColors;
use plugins::warp::WarpState;
use render;
use sim::CarID;
use std::collections::HashMap;
use std::process;

// TODO ideally these would be tuned kind of dynamically based on rendering speed
const MIN_ZOOM_FOR_ROADS: f64 = 0.15;
const MIN_ZOOM_FOR_PARCELS: f64 = 1.0;
const MIN_ZOOM_FOR_MOUSEOVER: f64 = 1.0;
const MIN_ZOOM_FOR_ROAD_MARKERS: f64 = 5.0;

pub struct UI {
    map: map_model::Map,
    draw_map: render::DrawMap,
    control_map: ControlMap,

    show_roads: ToggleableLayer,
    show_buildings: ToggleableLayer,
    show_intersections: ToggleableLayer,
    show_parcels: ToggleableLayer,
    show_icons: ToggleableLayer,
    debug_mode: ToggleableLayer,

    // This is a particularly special plugin, since it's always kind of active and other things
    // read/write it.
    current_selection_state: SelectionState,

    hider: Hider,
    current_search_state: SearchState,
    warp: WarpState,
    floodfiller: Floodfiller,
    steepness_viz: SteepnessVisualizer,
    osm_classifier: OsmClassifier,
    turn_colors: TurnColors,
    traffic_signal_editor: TrafficSignalEditor,
    stop_sign_editor: StopSignEditor,
    sim_ctrl: SimController,
    color_picker: ColorPicker,
    geom_validator: Validator,

    canvas: Canvas,
    // TODO maybe never pass this to other places? Always resolve colors here?
    cs: ColorScheme,
}

impl UI {
    pub fn new(abst_path: &str, window_size: Size, rng_seed: Option<u8>) -> UI {
        println!("Opening {}", abst_path);
        let map = map_model::Map::new(abst_path).expect("Couldn't load map");
        let (draw_map, center_pt) = render::DrawMap::new(&map);
        let control_map = ControlMap::new(&map);

        let steepness_viz = SteepnessVisualizer::new(&map);
        let turn_colors = TurnColors::new(&control_map);
        let sim_ctrl = SimController::new(&map, rng_seed);

        let mut ui = UI {
            map,
            draw_map,
            control_map,
            steepness_viz,
            turn_colors,
            sim_ctrl,

            show_roads: ToggleableLayer::new("roads", Key::D3, "3", Some(MIN_ZOOM_FOR_ROADS)),
            show_buildings: ToggleableLayer::new("buildings", Key::D1, "1", Some(0.0)),
            show_intersections: ToggleableLayer::new(
                "intersections",
                Key::D2,
                "2",
                Some(MIN_ZOOM_FOR_ROADS),
            ),
            show_parcels: ToggleableLayer::new("parcels", Key::D4, "4", Some(MIN_ZOOM_FOR_PARCELS)),
            show_icons: ToggleableLayer::new(
                "turn icons",
                Key::D7,
                "7",
                Some(MIN_ZOOM_FOR_ROAD_MARKERS),
            ),
            debug_mode: ToggleableLayer::new("debug mode", Key::G, "G", None),

            current_selection_state: SelectionState::Empty,
            hider: Hider::new(),
            current_search_state: SearchState::Empty,
            warp: WarpState::Empty,
            floodfiller: Floodfiller::new(),
            osm_classifier: OsmClassifier::new(),
            traffic_signal_editor: TrafficSignalEditor::new(),
            stop_sign_editor: StopSignEditor::new(),
            color_picker: ColorPicker::new(),
            geom_validator: Validator::new(),

            canvas: Canvas::new(window_size),
            cs: ColorScheme::load("color_scheme").unwrap(),
        };

        match abstutil::read_json::<EditorState>("editor_state") {
            Ok(state) => {
                println!("Loaded previous editor_state");
                ui.canvas.cam_x = state.cam_x;
                ui.canvas.cam_y = state.cam_y;
                ui.canvas.cam_zoom = state.cam_zoom;
                ui.control_map
                    .load_savestate(&state.traffic_signals, &state.stop_signs);
            }
            Err(_) => {
                println!("Couldn't load editor_state, just centering initial view");
                ui.canvas.center_on_map_pt(center_pt.x(), center_pt.y());
            }
        }

        let new_zoom = ui.canvas.cam_zoom;
        ui.zoom_for_toggleable_layers(-1.0, new_zoom);

        ui
    }

    // TODO or make a custom event for zoom change
    fn zoom_for_toggleable_layers(&mut self, old_zoom: f64, new_zoom: f64) {
        self.show_roads.handle_zoom(old_zoom, new_zoom);
        self.show_buildings.handle_zoom(old_zoom, new_zoom);
        self.show_intersections.handle_zoom(old_zoom, new_zoom);
        self.show_parcels.handle_zoom(old_zoom, new_zoom);
        self.show_icons.handle_zoom(old_zoom, new_zoom);
        self.debug_mode.handle_zoom(old_zoom, new_zoom);
    }

    fn mouseover_something(&self) -> Option<ID> {
        let (x, y) = self.canvas.get_cursor_in_map_space();

        let screen_bbox = self.canvas.get_screen_bbox();

        let roads_onscreen = if self.show_roads.is_enabled() {
            self.draw_map.get_roads_onscreen(screen_bbox, &self.hider)
        } else {
            Vec::new()
        };
        for r in &roads_onscreen {
            for c in &self.sim_ctrl.sim.get_draw_cars_on_road(r.id, &self.map) {
                if c.contains_pt(x, y) {
                    return Some(ID::Car(c.id));
                }
            }
        }

        if self.show_icons.is_enabled() {
            for t in &self.draw_map.get_turn_icons_onscreen(screen_bbox) {
                if t.contains_pt(x, y) {
                    return Some(ID::Turn(t.id));
                }
            }
        }

        if self.show_intersections.is_enabled() {
            for i in &self.draw_map
                .get_intersections_onscreen(screen_bbox, &self.hider)
            {
                for t in &self.map.get_i(i.id).turns {
                    for c in &self.sim_ctrl.sim.get_draw_cars_on_turn(*t, &self.map) {
                        if c.contains_pt(x, y) {
                            return Some(ID::Car(c.id));
                        }
                    }
                }

                if i.contains_pt(x, y) {
                    return Some(ID::Intersection(i.id));
                }
            }
        }

        if self.show_roads.is_enabled() {
            for r in &roads_onscreen {
                for c in &self.sim_ctrl.sim.get_draw_cars_on_road(r.id, &self.map) {
                    if c.contains_pt(x, y) {
                        return Some(ID::Car(c.id));
                    }
                }

                if r.road_contains_pt(x, y) {
                    return Some(ID::Road(r.id));
                }
            }
        }

        if self.show_buildings.is_enabled() {
            for b in &self.draw_map
                .get_buildings_onscreen(screen_bbox, &self.hider)
            {
                if b.contains_pt(x, y) {
                    return Some(ID::Building(b.id));
                }
            }
        }

        None
    }

    fn color_road(&self, id: map_model::RoadID) -> Color {
        let r = self.map.get_r(id);
        let mut default = match r.lane_type {
            map_model::LaneType::Driving => self.cs.get(Colors::Road),
            map_model::LaneType::Parking => self.cs.get(Colors::Parking),
            map_model::LaneType::Sidewalk => self.cs.get(Colors::Sidewalk),
        };
        if r.probably_broken {
            default = self.cs.get(Colors::Broken);
        }

        // TODO This evaluates all the color methods, which may be expensive. But the option
        // chaining is harder to read. :(
        vec![
            self.current_selection_state.color_r(r, &self.cs),
            self.current_search_state.color_r(r, &self.cs),
            self.floodfiller.color_r(r, &self.cs),
            self.steepness_viz.color_r(&self.map, r),
            self.osm_classifier.color_r(r, &self.cs),
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

        self.current_selection_state
            .color_i(i, &self.cs)
            .unwrap_or(default_color)
    }

    fn color_turn_icon(&self, id: map_model::TurnID) -> Color {
        let t = self.map.get_t(id);
        // TODO traffic signal selection logic maybe moves here
        self.current_selection_state
            .color_t(t, &self.cs)
            .unwrap_or_else(|| {
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
            self.current_selection_state.color_b(b, &self.cs),
            self.current_search_state.color_b(b, &self.cs),
            self.osm_classifier.color_b(b, &self.cs),
        ].iter()
            .filter_map(|c| *c)
            .next()
            .unwrap_or(self.cs.get(Colors::Building))
    }

    // Returns (boundary, fill) color
    fn color_parcel(&self, id: map_model::ParcelID) -> (Color, Color) {
        let _p = self.map.get_p(id);
        (
            self.cs.get(Colors::ParcelBoundary),
            self.cs.get(Colors::ParcelInterior),
        )
    }

    fn color_car(&self, id: CarID) -> Color {
        if let Some(c) = self.current_selection_state.color_c(id, &self.cs) {
            return c;
        }
        if self.sim_ctrl.sim.is_moving(id) {
            self.cs.get(Colors::MovingCar)
        } else {
            self.cs.get(Colors::StuckCar)
        }
    }
}

impl gui::GUI for UI {
    fn event(mut self, input: &mut UserInput) -> (UI, gui::EventLoopMode) {
        // First update the camera and handle zoom
        let old_zoom = self.canvas.cam_zoom;
        self.canvas.handle_event(input.use_event_directly());
        let new_zoom = self.canvas.cam_zoom;
        self.zoom_for_toggleable_layers(old_zoom, new_zoom);

        // Always handle mouseover
        if old_zoom >= MIN_ZOOM_FOR_MOUSEOVER && new_zoom < MIN_ZOOM_FOR_MOUSEOVER {
            self.current_selection_state = SelectionState::Empty;
        }
        if !self.canvas.is_dragging() && input.use_event_directly().mouse_cursor_args().is_some()
            && new_zoom >= MIN_ZOOM_FOR_MOUSEOVER
        {
            let item = self.mouseover_something();
            self.current_selection_state = self.current_selection_state.handle_mouseover(item);
        }

        if self.traffic_signal_editor.event(
            input,
            &self.map,
            &mut self.control_map,
            &self.current_selection_state,
        ) {
            return (self, gui::EventLoopMode::InputOnly);
        }

        if self.stop_sign_editor.event(
            input,
            &self.map,
            &mut self.control_map,
            &self.current_selection_state,
        ) {
            return (self, gui::EventLoopMode::InputOnly);
        }

        if self.color_picker
            .handle_event(input, &mut self.canvas, &mut self.cs)
        {
            return (self, gui::EventLoopMode::InputOnly);
        }

        if self.current_search_state.event(input) {
            return (self, gui::EventLoopMode::InputOnly);
        }

        {
            let (new_warp, active) = self.warp.event(
                input,
                &self.map,
                &mut self.canvas,
                &mut self.current_selection_state,
            );
            self.warp = new_warp;
            if active {
                return (self, gui::EventLoopMode::InputOnly);
            }
        }

        if self.show_roads.handle_event(input) {
            if let SelectionState::SelectedRoad(_, _) = self.current_selection_state {
                self.current_selection_state = SelectionState::Empty;
            }
            if let SelectionState::TooltipRoad(_) = self.current_selection_state {
                self.current_selection_state = SelectionState::Empty;
            }
            return (self, gui::EventLoopMode::InputOnly);
        }

        if self.show_buildings.handle_event(input) {
            if let SelectionState::SelectedBuilding(_) = self.current_selection_state {
                self.current_selection_state = SelectionState::Empty;
            }
            return (self, gui::EventLoopMode::InputOnly);
        }

        if self.show_intersections.handle_event(input) {
            if let SelectionState::SelectedIntersection(_) = self.current_selection_state {
                self.current_selection_state = SelectionState::Empty;
            }
            return (self, gui::EventLoopMode::InputOnly);
        }

        if self.show_parcels.handle_event(input) {
            return (self, gui::EventLoopMode::InputOnly);
        }

        if self.show_icons.handle_event(input) {
            return (self, gui::EventLoopMode::InputOnly);
        }

        if self.debug_mode.handle_event(input) {
            return (self, gui::EventLoopMode::InputOnly);
        }

        if self.steepness_viz.handle_event(input) {
            return (self, gui::EventLoopMode::InputOnly);
        }

        if self.osm_classifier.handle_event(input) {
            return (self, gui::EventLoopMode::InputOnly);
        }

        if self.hider.event(input, &mut self.current_selection_state) {
            return (self, gui::EventLoopMode::InputOnly);
        }

        if self.floodfiller.event(&self.map, input) {
            return (self, gui::EventLoopMode::InputOnly);
        }

        if self.geom_validator
            .event(input, &mut self.canvas, &self.map)
        {
            return (self, gui::EventLoopMode::InputOnly);
        }
        if input.unimportant_key_pressed(Key::I, "Validate map geometry") {
            self.geom_validator = Validator::start(&self.draw_map);
            return (self, gui::EventLoopMode::InputOnly);
        }

        if input.unimportant_key_pressed(Key::S, "Spawn 1000 cars in random places") {
            self.sim_ctrl.sim.spawn_many_on_empty_roads(&self.map, 1000);
            return (self, gui::EventLoopMode::InputOnly);
        }

        match self.current_selection_state {
            SelectionState::SelectedCar(id) => {
                // TODO not sure if we should debug like this (pushing the bit down to all the
                // layers representing an entity) or by using some scary global mutable singleton
                if input.unimportant_key_pressed(Key::D, "press D to debug") {
                    self.sim_ctrl.sim.toggle_debug(id);
                    return (self, gui::EventLoopMode::InputOnly);
                }
            }
            SelectionState::SelectedRoad(id, _) => {
                if input.key_pressed(Key::F, "Press F to start floodfilling from this road") {
                    self.floodfiller = Floodfiller::start(id);
                    return (self, gui::EventLoopMode::InputOnly);
                }

                if self.map.get_r(id).lane_type == map_model::LaneType::Driving {
                    if input.key_pressed(Key::A, "Press A to add a car starting from this road") {
                        if !self.sim_ctrl.sim.spawn_one_on_road(id) {
                            println!("No room, sorry");
                        }
                        return (self, gui::EventLoopMode::InputOnly);
                    }
                }
            }
            SelectionState::SelectedIntersection(id) => {
                if self.control_map.traffic_signals.contains_key(&id) {
                    if input.key_pressed(
                        Key::E,
                        &format!("Press E to edit traffic signal for {:?}", id),
                    ) {
                        self.traffic_signal_editor = TrafficSignalEditor::start(id);
                        return (self, gui::EventLoopMode::InputOnly);
                    }
                }
                if self.control_map.stop_signs.contains_key(&id) {
                    if input.key_pressed(Key::E, &format!("Press E to edit stop sign for {:?}", id))
                    {
                        self.stop_sign_editor = StopSignEditor::start(id);
                        return (self, gui::EventLoopMode::InputOnly);
                    }
                }
            }
            _ => {}
        }

        // Do this one lastish, since it conflicts with lots of other stuff
        {
            let (new_selection, active) = self.current_selection_state.event(input, &self.map);
            self.current_selection_state = new_selection;
            if active {
                return (self, gui::EventLoopMode::InputOnly);
            }
        }

        if input.unimportant_key_pressed(Key::Escape, "Press escape to quit") {
            let state = EditorState {
                cam_x: self.canvas.cam_x,
                cam_y: self.canvas.cam_y,
                cam_zoom: self.canvas.cam_zoom,
                traffic_signals: self.control_map.get_traffic_signals_savestate(),
                stop_signs: self.control_map.get_stop_signs_savestate(),
            };
            // TODO maybe make state line up with the map, so loading from a new map doesn't break
            abstutil::write_json("editor_state", &state).expect("Saving editor_state failed");
            abstutil::write_json("color_scheme", &self.cs).expect("Saving color_scheme failed");
            println!("Saved editor_state and color_scheme");
            process::exit(0);
        }

        // Sim controller plugin is kind of always active? If nothing else ran, let it use keys.
        if self.sim_ctrl.event(input, &self.map, &self.control_map) {
            (self, gui::EventLoopMode::Animation)
        } else {
            (self, gui::EventLoopMode::InputOnly)
        }
    }

    // TODO Weird to mut self just to set window_size on the canvas
    fn draw(&mut self, g: &mut GfxCtx, input: UserInput, window_size: Size) {
        g.clear(self.cs.get(Colors::Background));
        self.canvas.start_drawing(g, window_size);

        let screen_bbox = self.canvas.get_screen_bbox();

        if self.show_parcels.is_enabled() {
            for p in &self.draw_map.get_parcels_onscreen(screen_bbox) {
                p.draw(g, self.color_parcel(p.id));
            }
        }

        let roads_onscreen = if self.show_roads.is_enabled() {
            self.draw_map.get_roads_onscreen(screen_bbox, &self.hider)
        } else {
            Vec::new()
        };
        for r in &roads_onscreen {
            r.draw(g, self.color_road(r.id));
            if self.canvas.cam_zoom >= MIN_ZOOM_FOR_ROAD_MARKERS {
                r.draw_detail(g, &self.cs);
            }
            if self.debug_mode.is_enabled() {
                r.draw_debug(g, &self.cs, self.map.get_r(r.id));
            }
        }

        if self.show_intersections.is_enabled() {
            for i in &self.draw_map
                .get_intersections_onscreen(screen_bbox, &self.hider)
            {
                i.draw(g, self.color_intersection(i.id), &self.cs);
            }
        }

        if self.show_icons.is_enabled() {
            for t in &self.draw_map.get_turn_icons_onscreen(screen_bbox) {
                t.draw_icon(g, self.color_turn_icon(t.id), &self.cs);
                for c in &self.sim_ctrl.sim.get_draw_cars_on_turn(t.id, &self.map) {
                    c.draw(g, self.color_car(c.id));
                }
            }
        }

        for r in &roads_onscreen {
            for c in &self.sim_ctrl.sim.get_draw_cars_on_road(r.id, &self.map) {
                c.draw(g, self.color_car(c.id));
            }
        }

        if self.show_buildings.is_enabled() {
            for b in &self.draw_map
                .get_buildings_onscreen(screen_bbox, &self.hider)
            {
                b.draw(
                    g,
                    self.color_building(b.id),
                    self.cs.get(Colors::BuildingPath),
                );
            }
        }

        self.current_selection_state.draw(
            &self.map,
            &self.canvas,
            &self.draw_map,
            &self.control_map,
            &self.sim_ctrl.sim,
            &self.cs,
            g,
        );

        self.color_picker.draw(&self.canvas, g);

        let mut osd_lines = self.sim_ctrl.get_osd_lines();
        let action_lines = input.get_possible_actions();
        if !action_lines.is_empty() {
            osd_lines.push(String::from(""));
            osd_lines.extend(action_lines);
        }
        let search_lines = self.current_search_state.get_osd_lines();
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
    pub cam_x: f64,
    pub cam_y: f64,
    pub cam_zoom: f64,

    pub traffic_signals: HashMap<IntersectionID, ModifiedTrafficSignal>,
    pub stop_signs: HashMap<IntersectionID, ModifiedStopSign>,
}
