// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

// TODO this should just be a way to handle interactions between plugins

use abstutil;
use colors::{ColorScheme, Colors};
use control::ControlMap;
use control::{ModifiedStopSign, ModifiedTrafficSignal};
use ezgui;
use ezgui::canvas::Canvas;
use ezgui::input::UserInput;
use ezgui::{GfxCtx, ToggleableLayer};
use geom::Pt2D;
use graphics::types::Color;
use gui;
use kml;
use map_model;
use map_model::IntersectionID;
use piston::input::{Key, MouseCursorEvent};
use piston::window::Size;
use plugins::classification::OsmClassifier;
use plugins::color_picker::ColorPicker;
use plugins::floodfill::Floodfiller;
use plugins::geom_validation::Validator;
use plugins::road_editor::RoadEditor;
use plugins::search::SearchState;
use plugins::selection::{Hider, SelectionState, ID};
use plugins::sim_controls::SimController;
use plugins::steep::SteepnessVisualizer;
use plugins::stop_sign_editor::StopSignEditor;
use plugins::traffic_signal_editor::TrafficSignalEditor;
use plugins::turn_colors::TurnColors;
use plugins::warp::WarpState;
use render;
use sim;
use sim::{CarID, CarState, PedestrianID};
use std::collections::HashMap;
use std::process;

// TODO ideally these would be tuned kind of dynamically based on rendering speed
const MIN_ZOOM_FOR_LANES: f64 = 0.15;
const MIN_ZOOM_FOR_PARCELS: f64 = 1.0;
const MIN_ZOOM_FOR_MOUSEOVER: f64 = 1.0;
const MIN_ZOOM_FOR_LANE_MARKERS: f64 = 5.0;

pub struct UI {
    map: map_model::Map,
    draw_map: render::DrawMap,
    control_map: ControlMap,

    show_lanes: ToggleableLayer,
    show_buildings: ToggleableLayer,
    show_intersections: ToggleableLayer,
    show_parcels: ToggleableLayer,
    show_extra_shapes: ToggleableLayer,
    show_all_turn_icons: ToggleableLayer,
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
    road_editor: RoadEditor,
    sim_ctrl: SimController,
    color_picker: ColorPicker,
    geom_validator: Validator,

    canvas: Canvas,
    // TODO maybe never pass this to other places? Always resolve colors here?
    cs: ColorScheme,
}

impl UI {
    pub fn new(
        load: String,
        scenario_name: String,
        rng_seed: Option<u8>,
        kml: Option<String>,
        window_size: Size,
    ) -> UI {
        let (map, edits, control_map, sim) = sim::load(load, scenario_name, rng_seed);

        let extra_shapes = if let Some(path) = kml {
            kml::load(&path, &map.get_gps_bounds()).expect("Couldn't load extra KML shapes")
        } else {
            Vec::new()
        };

        let (draw_map, center_pt) = render::DrawMap::new(&map, &control_map, extra_shapes);

        let steepness_viz = SteepnessVisualizer::new(&map);
        let turn_colors = TurnColors::new(&control_map);
        let sim_ctrl = SimController::new(sim);

        let mut ui = UI {
            map,
            draw_map,
            control_map,
            steepness_viz,
            turn_colors,
            sim_ctrl,

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

            current_selection_state: SelectionState::Empty,
            hider: Hider::new(),
            current_search_state: SearchState::Empty,
            warp: WarpState::Empty,
            floodfiller: Floodfiller::new(),
            osm_classifier: OsmClassifier::new(),
            traffic_signal_editor: TrafficSignalEditor::new(),
            stop_sign_editor: StopSignEditor::new(),
            road_editor: RoadEditor::new(edits),
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
        self.show_lanes.handle_zoom(old_zoom, new_zoom);
        self.show_buildings.handle_zoom(old_zoom, new_zoom);
        self.show_intersections.handle_zoom(old_zoom, new_zoom);
        self.show_parcels.handle_zoom(old_zoom, new_zoom);
        self.show_extra_shapes.handle_zoom(old_zoom, new_zoom);
        self.show_all_turn_icons.handle_zoom(old_zoom, new_zoom);
        self.debug_mode.handle_zoom(old_zoom, new_zoom);
    }

    fn mouseover_something(&self) -> Option<ID> {
        let (x, y) = self.canvas.get_cursor_in_map_space();
        let pt = Pt2D::new(x, y);

        let screen_bbox = self.canvas.get_screen_bbox();

        if self.show_extra_shapes.is_enabled() {
            for s in &self.draw_map
                .get_extra_shapes_onscreen(screen_bbox, &self.hider)
            {
                if s.contains_pt(pt) {
                    return Some(ID::ExtraShape(s.id));
                }
            }
        }

        let lanes_onscreen = if self.show_lanes.is_enabled() {
            self.draw_map.get_loads_onscreen(screen_bbox, &self.hider)
        } else {
            Vec::new()
        };
        for l in &lanes_onscreen {
            for c in &self.sim_ctrl.sim.get_draw_cars_on_lane(l.id, &self.map) {
                if c.contains_pt(pt) {
                    return Some(ID::Car(c.id));
                }
            }
            for p in &self.sim_ctrl.sim.get_draw_peds_on_lane(l.id, &self.map) {
                if p.contains_pt(pt) {
                    return Some(ID::Pedestrian(p.id));
                }
            }
        }

        if self.show_intersections.is_enabled() {
            for i in &self.draw_map
                .get_intersections_onscreen(screen_bbox, &self.hider)
            {
                let show_icons = self.show_icons_for(i.id);

                for t in &self.map.get_i(i.id).turns {
                    if show_icons && self.draw_map.get_t(*t).contains_pt(pt) {
                        return Some(ID::Turn(*t));
                    }

                    for c in &self.sim_ctrl.sim.get_draw_cars_on_turn(*t, &self.map) {
                        if c.contains_pt(pt) {
                            return Some(ID::Car(c.id));
                        }
                    }
                    for p in &self.sim_ctrl.sim.get_draw_peds_on_turn(*t, &self.map) {
                        if p.contains_pt(pt) {
                            return Some(ID::Pedestrian(p.id));
                        }
                    }
                }

                if i.contains_pt(pt) {
                    return Some(ID::Intersection(i.id));
                }
            }
        }

        if self.show_lanes.is_enabled() {
            for l in &lanes_onscreen {
                if l.contains_pt(pt) {
                    return Some(ID::Lane(l.id));
                }
            }
        }

        if self.show_buildings.is_enabled() {
            for b in &self.draw_map
                .get_buildings_onscreen(screen_bbox, &self.hider)
            {
                if b.contains_pt(pt) {
                    return Some(ID::Building(b.id));
                }
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
            self.current_selection_state.color_l(l, &self.cs),
            self.current_search_state.color_l(l, &self.map, &self.cs),
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
        (
            self.cs.get(Colors::ParcelBoundary),
            COLORS[p.block % COLORS.len()],
        )
    }

    fn color_car(&self, id: CarID) -> Color {
        if let Some(c) = self.current_selection_state.color_c(id, &self.cs) {
            return c;
        }
        match self.sim_ctrl.sim.get_car_state(id) {
            CarState::Debug => ezgui::shift_color(self.cs.get(Colors::DebugCar), id.0),
            CarState::Moving => ezgui::shift_color(self.cs.get(Colors::MovingCar), id.0),
            CarState::Stuck => ezgui::shift_color(self.cs.get(Colors::StuckCar), id.0),
            CarState::Parked => ezgui::shift_color(self.cs.get(Colors::ParkedCar), id.0),
        }
    }

    fn color_ped(&self, id: PedestrianID) -> Color {
        if let Some(c) = self.current_selection_state.color_p(id, &self.cs) {
            return c;
        }
        ezgui::shift_color(self.cs.get(Colors::Pedestrian), id.0)
    }

    fn show_icons_for(&self, id: IntersectionID) -> bool {
        self.show_all_turn_icons.is_enabled()
            || self.stop_sign_editor.show_turn_icons(id)
            || self.traffic_signal_editor.show_turn_icons(id)
    }
}

impl gui::GUI for UI {
    fn event(&mut self, input: &mut UserInput) -> gui::EventLoopMode {
        // First update the camera and handle zoom
        let old_zoom = self.canvas.cam_zoom;
        self.canvas.handle_event(input.use_event_directly());
        let new_zoom = self.canvas.cam_zoom;
        self.zoom_for_toggleable_layers(old_zoom, new_zoom);

        // Always handle mouseover
        if old_zoom >= MIN_ZOOM_FOR_MOUSEOVER && new_zoom < MIN_ZOOM_FOR_MOUSEOVER {
            self.current_selection_state = SelectionState::Empty;
        }
        if !self.canvas.is_dragging()
            && input.use_event_directly().mouse_cursor_args().is_some()
            && new_zoom >= MIN_ZOOM_FOR_MOUSEOVER
        {
            let item = self.mouseover_something();
            self.current_selection_state = self.current_selection_state.handle_mouseover(item);
        }

        // Run each plugin, short-circuiting if the plugin claimed it was active.
        macro_rules! stop_if_done {
            ($plugin:expr) => {
                if $plugin {
                    return gui::EventLoopMode::InputOnly;
                }
            };
        }

        stop_if_done!(self.traffic_signal_editor.event(
            input,
            &self.map,
            &mut self.control_map,
            &self.current_selection_state,
        ));
        stop_if_done!(self.stop_sign_editor.event(
            input,
            &self.map,
            &mut self.control_map,
            &self.current_selection_state,
        ));
        stop_if_done!(self.road_editor.event(
            input,
            &self.current_selection_state,
            &mut self.map,
            &mut self.draw_map,
            &self.control_map,
            &mut self.sim_ctrl.sim
        ));
        stop_if_done!(self.current_search_state.event(input));
        stop_if_done!(self.warp.event(
            input,
            &self.map,
            &self.sim_ctrl.sim,
            &mut self.canvas,
            &mut self.current_selection_state,
        ));
        stop_if_done!(
            self.color_picker
                .handle_event(input, &mut self.canvas, &mut self.cs)
        );

        if self.show_lanes.handle_event(input) {
            if let SelectionState::SelectedLane(_, _) = self.current_selection_state {
                self.current_selection_state = SelectionState::Empty;
            }
            if let SelectionState::Tooltip(ID::Lane(_)) = self.current_selection_state {
                self.current_selection_state = SelectionState::Empty;
            }
            return gui::EventLoopMode::InputOnly;
        }
        if self.show_buildings.handle_event(input) {
            if let SelectionState::SelectedBuilding(_) = self.current_selection_state {
                self.current_selection_state = SelectionState::Empty;
            }
            if let SelectionState::Tooltip(ID::Building(_)) = self.current_selection_state {
                self.current_selection_state = SelectionState::Empty;
            }
            return gui::EventLoopMode::InputOnly;
        }
        if self.show_intersections.handle_event(input) {
            if let SelectionState::SelectedIntersection(_) = self.current_selection_state {
                self.current_selection_state = SelectionState::Empty;
            }
            if let SelectionState::Tooltip(ID::Intersection(_)) = self.current_selection_state {
                self.current_selection_state = SelectionState::Empty;
            }
            return gui::EventLoopMode::InputOnly;
        }
        if self.show_extra_shapes.handle_event(input) {
            if let SelectionState::SelectedExtraShape(_) = self.current_selection_state {
                self.current_selection_state = SelectionState::Empty;
            }
            if let SelectionState::Tooltip(ID::ExtraShape(_)) = self.current_selection_state {
                self.current_selection_state = SelectionState::Empty;
            }
            return gui::EventLoopMode::InputOnly;
        }
        if self.show_all_turn_icons.handle_event(input) {
            if let SelectionState::SelectedTurn(_) = self.current_selection_state {
                self.current_selection_state = SelectionState::Empty;
            }
            if let SelectionState::Tooltip(ID::Turn(_)) = self.current_selection_state {
                self.current_selection_state = SelectionState::Empty;
            }
            return gui::EventLoopMode::InputOnly;
        }

        stop_if_done!(self.show_parcels.handle_event(input));
        stop_if_done!(self.debug_mode.handle_event(input));
        stop_if_done!(self.steepness_viz.handle_event(input));
        stop_if_done!(self.osm_classifier.handle_event(input));
        stop_if_done!(self.hider.event(input, &mut self.current_selection_state));
        stop_if_done!(self.floodfiller.event(&self.map, input));
        stop_if_done!(
            self.geom_validator
                .event(input, &mut self.canvas, &self.map)
        );

        if input.unimportant_key_pressed(Key::I, "Validate map geometry") {
            self.geom_validator = Validator::start(&self.draw_map);
            return gui::EventLoopMode::InputOnly;
        }
        if input.unimportant_key_pressed(Key::S, "Seed the map with agents") {
            self.sim_ctrl.sim.seed_parked_cars(0.5);
            self.sim_ctrl.sim.seed_walking_trips(&self.map, 100);
            self.sim_ctrl.sim.seed_driving_trips(&self.map, 100);
            return gui::EventLoopMode::InputOnly;
        }

        match self.current_selection_state {
            SelectionState::SelectedCar(id) => {
                // TODO not sure if we should debug like this (pushing the bit down to all the
                // layers representing an entity) or by using some scary global mutable singleton
                if input.unimportant_key_pressed(Key::D, "debug") {
                    self.sim_ctrl.sim.toggle_debug(id);
                    return gui::EventLoopMode::InputOnly;
                }
                if input.key_pressed(Key::A, "start this parked car") {
                    self.sim_ctrl.sim.start_parked_car(&self.map, id);
                    return gui::EventLoopMode::InputOnly;
                }
            }
            SelectionState::SelectedLane(id, _) => {
                if input.key_pressed(Key::F, "start floodfilling from this lane") {
                    self.floodfiller = Floodfiller::start(id);
                    return gui::EventLoopMode::InputOnly;
                }

                if self.map.get_l(id).is_sidewalk()
                    && input.key_pressed(Key::A, "spawn a pedestrian here")
                {
                    self.sim_ctrl.sim.spawn_pedestrian(&self.map, id);
                    return gui::EventLoopMode::InputOnly;
                }
            }
            SelectionState::SelectedIntersection(id) => {
                if self.control_map.traffic_signals.contains_key(&id) {
                    if input.key_pressed(Key::E, &format!("edit traffic signal for {:?}", id)) {
                        self.traffic_signal_editor = TrafficSignalEditor::start(id);
                        return gui::EventLoopMode::InputOnly;
                    }
                }
                if self.control_map.stop_signs.contains_key(&id) {
                    if input.key_pressed(Key::E, &format!("edit stop sign for {:?}", id)) {
                        self.stop_sign_editor = StopSignEditor::start(id);
                        return gui::EventLoopMode::InputOnly;
                    }
                }
            }
            _ => {}
        }

        // Do this one lastish, since it conflicts with lots of other stuff
        stop_if_done!(self.current_selection_state.event(
            input,
            &self.map,
            &mut self.sim_ctrl.sim,
            &self.control_map
        ));

        if input.unimportant_key_pressed(Key::Escape, "quit") {
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
            abstutil::write_json("road_edits.json", self.road_editor.get_edits())
                .expect("Saving road_edits.json failed");
            println!("Saved editor_state, color_scheme, and road_edits.json");
            process::exit(0);
        }

        // Sim controller plugin is kind of always active? If nothing else ran, let it use keys.
        if self.sim_ctrl.event(input, &self.map, &self.control_map) {
            gui::EventLoopMode::Animation
        } else {
            gui::EventLoopMode::InputOnly
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

        let lanes_onscreen = if self.show_lanes.is_enabled() {
            self.draw_map.get_loads_onscreen(screen_bbox, &self.hider)
        } else {
            Vec::new()
        };
        for l in &lanes_onscreen {
            l.draw(g, self.color_lane(l.id));
            if self.canvas.cam_zoom >= MIN_ZOOM_FOR_LANE_MARKERS {
                l.draw_detail(g, &self.canvas, &self.cs);
            }
            if self.debug_mode.is_enabled() {
                l.draw_debug(g, &self.cs, self.map.get_l(l.id));
            }
        }

        if self.show_intersections.is_enabled() {
            for i in &self.draw_map
                .get_intersections_onscreen(screen_bbox, &self.hider)
            {
                i.draw(g, self.color_intersection(i.id), &self.cs);
                let show_icons = self.show_icons_for(i.id);
                for t in &self.map.get_i(i.id).turns {
                    if show_icons {
                        self.draw_map
                            .get_t(*t)
                            .draw_icon(g, self.color_turn_icon(*t), &self.cs);
                    }
                    for c in &self.sim_ctrl.sim.get_draw_cars_on_turn(*t, &self.map) {
                        c.draw(g, self.color_car(c.id));
                    }
                    for p in &self.sim_ctrl.sim.get_draw_peds_on_turn(*t, &self.map) {
                        p.draw(g, self.color_ped(p.id));
                    }
                }
            }
        }

        // Building paths overlap sidewalks, so do these first to not look messy
        if self.show_buildings.is_enabled() {
            for b in &self.draw_map
                .get_buildings_onscreen(screen_bbox, &self.hider)
            {
                b.draw(
                    g,
                    self.color_building(b.id),
                    self.cs.get(Colors::BuildingPath),
                    self.cs.get(Colors::BuildingBoundary),
                );
            }
        }

        for l in &lanes_onscreen {
            for c in &self.sim_ctrl.sim.get_draw_cars_on_lane(l.id, &self.map) {
                c.draw(g, self.color_car(c.id));
            }
            for p in &self.sim_ctrl.sim.get_draw_peds_on_lane(l.id, &self.map) {
                p.draw(g, self.color_ped(p.id));
            }
        }

        if self.show_extra_shapes.is_enabled() {
            for s in &self.draw_map
                .get_extra_shapes_onscreen(screen_bbox, &self.hider)
            {
                // TODO no separate color method?
                s.draw(
                    g,
                    self.current_selection_state
                        .color_es(s.id, &self.cs)
                        .unwrap_or(self.cs.get(Colors::ExtraShape)),
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
