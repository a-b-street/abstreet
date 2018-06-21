// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

// TODO this should just be a way to handle interactions between plugins

extern crate map_model;

use animation;
use colors::ColorScheme;
use control::ControlMap;
use ezgui::ToggleableLayer;
use ezgui::canvas;
use ezgui::canvas::{Canvas, GfxCtx};
use ezgui::input::UserInput;
use geom;
use graphics::types::Color;
use piston::input::{Key, MouseCursorEvent};
use piston::window::Size;
use plugins::classification::OsmClassifier;
use plugins::color_picker::ColorPicker;
use plugins::floodfill::Floodfiller;
use plugins::search::SearchState;
use plugins::selection::{SelectionState, ID};
use plugins::sim_controls::SimController;
use plugins::steep::SteepnessVisualizer;
use plugins::stop_sign_editor::StopSignEditor;
use plugins::traffic_signal_editor::TrafficSignalEditor;
use plugins::turn_colors::TurnColors;
use render;
use savestate;
use sim::CarID;
use std::process;

// TODO ideally these would be tuned kind of dynamically based on rendering speed
const MIN_ZOOM_FOR_ROADS: f64 = 0.15;
const MIN_ZOOM_FOR_PARCELS: f64 = 1.0;
const MIN_ZOOM_FOR_MOUSEOVER: f64 = 1.0;
const MIN_ZOOM_FOR_ROAD_MARKERS: f64 = 5.0;

pub struct UI {
    map: map_model::Map,
    geom_map: geom::GeomMap,
    draw_map: render::DrawMap,
    control_map: ControlMap,

    show_roads: ToggleableLayer,
    show_buildings: ToggleableLayer,
    show_intersections: ToggleableLayer,
    show_parcels: ToggleableLayer,
    show_icons: ToggleableLayer,
    // TODO should these be more associated with their plugins?
    steepness_active: ToggleableLayer,
    osm_classifier_active: ToggleableLayer,
    debug_mode: ToggleableLayer,

    current_selection_state: SelectionState,
    current_search_state: SearchState,
    floodfiller: Option<Floodfiller>,
    steepness_viz: SteepnessVisualizer,
    osm_classifier: OsmClassifier,
    turn_colors: TurnColors,
    traffic_signal_editor: Option<TrafficSignalEditor>,
    stop_sign_editor: Option<StopSignEditor>,
    sim_ctrl: SimController,
    color_picker: ColorPicker,

    canvas: Canvas,
    color_scheme: ColorScheme,
}

impl UI {
    pub fn new(abst_path: &str, window_size: &Size, rng_seed: Option<u8>) -> UI {
        println!("Opening {}", abst_path);
        let data = map_model::load_pb(abst_path).expect("Couldn't load pb");
        let map = map_model::Map::new(&data);
        let geom_map = geom::GeomMap::new(&map);
        let (draw_map, _, center_pt) = render::DrawMap::new(&map, &geom_map);
        let control_map = ControlMap::new(&map, &geom_map);

        let steepness_viz = SteepnessVisualizer::new(&map);
        let turn_colors = TurnColors::new(&control_map);
        let sim_ctrl = SimController::new(&map, &geom_map, rng_seed);

        let mut ui = UI {
            map,
            geom_map,
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
            steepness_active: ToggleableLayer::new("steepness visualize", Key::D5, "5", None),
            osm_classifier_active: ToggleableLayer::new("OSM type classifier", Key::D6, "6", None),
            debug_mode: ToggleableLayer::new("debug mode", Key::G, "G", None),

            current_selection_state: SelectionState::Empty,
            current_search_state: SearchState::Empty,
            floodfiller: None,
            osm_classifier: OsmClassifier {},
            traffic_signal_editor: None,
            stop_sign_editor: None,
            color_picker: ColorPicker::new(),

            canvas: Canvas::new(),
            color_scheme: ColorScheme::random_settings(),
        };

        match savestate::load("editor_state") {
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
                ui.canvas
                    .center_on_map_pt(center_pt.x(), center_pt.y(), window_size);
            }
        }

        match ColorScheme::load("color_scheme") {
            Ok(scheme) => {
                println!("Loaded previous color_scheme");
                ui.color_scheme = scheme;
            }
            Err(err) => {
                println!("Couldn't load color_scheme: {}", err);
            }
        }

        // TODO or make a custom event for zoom change
        let old_zoom = -1.0;
        let new_zoom = ui.canvas.cam_zoom;
        ui.show_roads.handle_zoom(old_zoom, new_zoom);
        ui.show_buildings.handle_zoom(old_zoom, new_zoom);
        ui.show_intersections.handle_zoom(old_zoom, new_zoom);
        ui.show_parcels.handle_zoom(old_zoom, new_zoom);
        ui.show_icons.handle_zoom(old_zoom, new_zoom);
        ui.steepness_active.handle_zoom(old_zoom, new_zoom);
        ui.osm_classifier_active.handle_zoom(old_zoom, new_zoom);
        ui.debug_mode.handle_zoom(old_zoom, new_zoom);

        ui
    }

    pub fn event(
        mut self,
        input: &mut UserInput,
        window_size: &Size,
    ) -> (UI, animation::EventLoopMode) {
        let mut event_loop_mode = animation::EventLoopMode::InputOnly;
        let mut edit_mode = false;

        if let Some(mut e) = self.traffic_signal_editor {
            edit_mode = true;
            if e.event(
                input,
                &self.map,
                &self.geom_map,
                &mut self.control_map,
                &self.current_selection_state,
            ) {
                self.traffic_signal_editor = None;
            } else {
                self.traffic_signal_editor = Some(e);
            }
        }

        if let Some(mut e) = self.stop_sign_editor {
            edit_mode = true;
            if e.event(
                input,
                &self.map,
                &self.geom_map,
                &mut self.control_map,
                &self.current_selection_state,
            ) {
                self.stop_sign_editor = None;
            } else {
                self.stop_sign_editor = Some(e);
            }
        }

        self.current_search_state = self.current_search_state.event(input);

        if !edit_mode
            && self.sim_ctrl
                .event(input, &self.geom_map, &self.map, &self.control_map)
        {
            event_loop_mode = event_loop_mode.merge(animation::EventLoopMode::Animation);
        }

        let old_zoom = self.canvas.cam_zoom;
        self.canvas.handle_event(input.use_event_directly());
        let new_zoom = self.canvas.cam_zoom;

        self.show_roads.handle_zoom(old_zoom, new_zoom);
        self.show_buildings.handle_zoom(old_zoom, new_zoom);
        self.show_intersections.handle_zoom(old_zoom, new_zoom);
        self.show_parcels.handle_zoom(old_zoom, new_zoom);
        self.show_icons.handle_zoom(old_zoom, new_zoom);
        self.steepness_active.handle_zoom(old_zoom, new_zoom);
        self.osm_classifier_active.handle_zoom(old_zoom, new_zoom);
        self.debug_mode.handle_zoom(old_zoom, new_zoom);

        if !edit_mode {
            if self.show_roads.handle_event(input) {
                if let SelectionState::SelectedRoad(_, _) = self.current_selection_state {
                    self.current_selection_state = SelectionState::Empty;
                }
                if let SelectionState::TooltipRoad(_) = self.current_selection_state {
                    self.current_selection_state = SelectionState::Empty;
                }
            }
            if self.show_buildings.handle_event(input) {
                if let SelectionState::SelectedBuilding(_) = self.current_selection_state {
                    self.current_selection_state = SelectionState::Empty;
                }
            }
            if self.show_intersections.handle_event(input) {
                if let SelectionState::SelectedIntersection(_) = self.current_selection_state {
                    self.current_selection_state = SelectionState::Empty;
                }
            }
            self.show_parcels.handle_event(input);
            self.show_icons.handle_event(input);
            self.steepness_active.handle_event(input);
            self.osm_classifier_active.handle_event(input);
            self.debug_mode.handle_event(input);
            self.color_picker = self.color_picker.handle_event(input, window_size);
        }

        if old_zoom >= MIN_ZOOM_FOR_MOUSEOVER && new_zoom < MIN_ZOOM_FOR_MOUSEOVER {
            self.current_selection_state = SelectionState::Empty;
        }
        if !self.canvas.is_dragging() && input.use_event_directly().mouse_cursor_args().is_some()
            && new_zoom >= MIN_ZOOM_FOR_MOUSEOVER
        {
            self.current_selection_state = self.current_selection_state
                .handle_mouseover(&self.mouseover_something(window_size));
        }
        // TODO can't get this destructuring expressed right
        let (new_selection_state, new_event_loop_mode) = self.current_selection_state
            .event(input, &mut self.sim_ctrl.sim);
        event_loop_mode = event_loop_mode.merge(new_event_loop_mode);
        self.current_selection_state = new_selection_state;
        match self.current_selection_state {
            SelectionState::SelectedRoad(id, _) => {
                if self.floodfiller.is_none() {
                    if input.key_pressed(Key::F, "Press F to start floodfilling from this road") {
                        self.floodfiller = Some(Floodfiller::new(id));
                    }
                }

                if self.map.get_r(id).lane_type == map_model::LaneType::Driving {
                    if input.key_pressed(Key::A, "Press A to add a car starting from this road") {
                        if !self.sim_ctrl.sim.spawn_one_on_road(id) {
                            println!("No room, sorry");
                        }
                    }
                }
            }
            SelectionState::SelectedIntersection(id) => {
                if self.traffic_signal_editor.is_none()
                    && self.control_map.traffic_signals.contains_key(&id)
                {
                    if input.key_pressed(Key::E, "Press E to edit this traffic signal") {
                        self.traffic_signal_editor = Some(TrafficSignalEditor::new(id));
                    }
                }
                if self.stop_sign_editor.is_none() && self.control_map.stop_signs.contains_key(&id)
                {
                    if input.key_pressed(Key::E, "Press E to edit this stop sign") {
                        self.stop_sign_editor = Some(StopSignEditor::new(id));
                    }
                }
            }
            _ => {}
        }

        if let Some(mut f) = self.floodfiller {
            if f.event(&self.map, input) {
                self.floodfiller = None;
            } else {
                self.floodfiller = Some(f);
            }
        }

        if input.unimportant_key_pressed(Key::S, "Spawn 1000 cars in random places") {
            self.sim_ctrl.sim.spawn_many_on_empty_roads(&self.map, 1000);
        }

        if input.unimportant_key_pressed(Key::Escape, "Press escape to quit") {
            let state = savestate::EditorState {
                cam_x: self.canvas.cam_x,
                cam_y: self.canvas.cam_y,
                cam_zoom: self.canvas.cam_zoom,
                traffic_signals: self.control_map.get_traffic_signals_savestate(),
                stop_signs: self.control_map.get_stop_signs_savestate(),
            };
            // TODO maybe make state line up with the map, so loading from a new map doesn't break
            savestate::write("editor_state", state).expect("Saving editor_state failed");
            self.color_scheme
                .write("color_scheme")
                .expect("Saving color_scheme failed");
            println!("Saved editor_state and color_scheme");
            process::exit(0);
        }

        (self, event_loop_mode)
    }

    pub fn draw(&self, g: &mut GfxCtx, input: UserInput) {
        g.ctx = self.canvas.get_transformed_context(&g.orig_ctx);

        let screen_bbox = self.canvas.get_screen_bbox(&g.window_size);

        let roads_onscreen = if self.show_roads.is_enabled() {
            self.draw_map.get_roads_onscreen(screen_bbox)
        } else {
            Vec::new()
        };
        for r in &roads_onscreen {
            r.draw(g, self.color_road(r.id));
            if self.canvas.cam_zoom >= MIN_ZOOM_FOR_ROAD_MARKERS {
                r.draw_detail(g);
            }
            if self.debug_mode.is_enabled() {
                r.draw_debug(g, self.geom_map.get_r(r.id));
            }
        }

        if self.show_intersections.is_enabled() {
            for i in &self.draw_map.get_intersections_onscreen(screen_bbox) {
                i.draw(g, self.color_intersection(i.id));
            }
        }

        if self.show_icons.is_enabled() {
            for t in &self.draw_map.get_turn_icons_onscreen(screen_bbox) {
                t.draw_icon(g, self.color_turn_icon(t.id));
                for c in &self.sim_ctrl
                    .sim
                    .get_draw_cars_on_turn(t.id, &self.geom_map)
                {
                    c.draw(g, self.color_car(c.id));
                }
            }
        }

        for r in &roads_onscreen {
            for c in &self.sim_ctrl
                .sim
                .get_draw_cars_on_road(r.id, &self.geom_map)
            {
                c.draw(g, self.color_car(c.id));
            }
        }

        if self.show_parcels.is_enabled() {
            for p in &self.draw_map.get_parcels_onscreen(screen_bbox) {
                p.draw(g, self.color_parcel(p.id));
            }
        }

        if self.show_buildings.is_enabled() {
            for b in &self.draw_map.get_buildings_onscreen(screen_bbox) {
                b.draw(g, self.color_building(b.id));
            }
        }

        self.current_selection_state.draw(
            &self.map,
            &self.canvas,
            &self.geom_map,
            &self.draw_map,
            &self.control_map,
            &self.sim_ctrl.sim,
            g,
        );

        self.color_picker.draw(g);

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
        self.canvas.draw_osd_notification(g, &osd_lines);
    }

    fn mouseover_something(&self, window_size: &Size) -> Option<ID> {
        let (x, y) = self.canvas.get_cursor_in_map_space();

        let screen_bbox = self.canvas.get_screen_bbox(window_size);

        let roads_onscreen = if self.show_roads.is_enabled() {
            self.draw_map.get_roads_onscreen(screen_bbox)
        } else {
            Vec::new()
        };
        for r in &roads_onscreen {
            for c in &self.sim_ctrl
                .sim
                .get_draw_cars_on_road(r.id, &self.geom_map)
            {
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
            for i in &self.draw_map.get_intersections_onscreen(screen_bbox) {
                for t in &self.map.get_i(i.id).turns {
                    for c in &self.sim_ctrl.sim.get_draw_cars_on_turn(*t, &self.geom_map) {
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
                for c in &self.sim_ctrl
                    .sim
                    .get_draw_cars_on_road(r.id, &self.geom_map)
                {
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
            for b in &self.draw_map.get_buildings_onscreen(screen_bbox) {
                if b.contains_pt(x, y) {
                    return Some(ID::Building(b.id));
                }
            }
        }

        None
    }

    fn color_road(&self, id: map_model::RoadID) -> Color {
        let r = self.map.get_r(id);
        let default = match r.lane_type {
            map_model::LaneType::Driving => render::ROAD_COLOR,
            map_model::LaneType::Parking => render::PARKING_COLOR,
            map_model::LaneType::Sidewalk => render::SIDEWALK_COLOR,
        };

        // TODO This evaluates all the color methods, which may be expensive. But the option
        // chaining is harder to read. :(
        vec![
            self.current_selection_state.color_r(r),
            self.current_search_state.color_r(r),
            self.floodfiller.as_ref().and_then(|f| f.color_r(r)),
            if self.steepness_active.is_enabled() {
                self.steepness_viz.color_r(&self.map, r)
            } else {
                None
            },
            if self.osm_classifier_active.is_enabled() {
                self.osm_classifier.color_r(r)
            } else {
                None
            },
        ].iter()
            .filter_map(|c| *c)
            .next()
            .unwrap_or(default)
    }

    fn color_intersection(&self, id: map_model::IntersectionID) -> Color {
        let i = self.map.get_i(id);
        // TODO weird to squeeze in some quick logic here?
        let default_color = if let Some(s) = self.control_map.traffic_signals.get(&i.id) {
            if s.changed() {
                render::CHANGED_TRAFFIC_SIGNAL_INTERSECTION_COLOR
            } else {
                render::TRAFFIC_SIGNAL_INTERSECTION_COLOR
            }
        } else if let Some(s) = self.control_map.stop_signs.get(&i.id) {
            if s.changed() {
                render::CHANGED_STOP_SIGN_INTERSECTION_COLOR
            } else {
                render::NORMAL_INTERSECTION_COLOR
            }
        } else {
            render::NORMAL_INTERSECTION_COLOR
        };

        self.current_selection_state
            .color_i(i)
            .unwrap_or(default_color)
    }

    fn color_turn_icon(&self, id: map_model::TurnID) -> Color {
        let t = self.map.get_t(id);
        // TODO traffic signal selection logic maybe moves here
        self.current_selection_state.color_t(t).unwrap_or_else(|| {
            self.stop_sign_editor
                .as_ref()
                .and_then(|e| e.color_t(t, &self.control_map))
                .unwrap_or_else(|| {
                    self.traffic_signal_editor
                        .as_ref()
                        .and_then(|e| e.color_t(t, &self.geom_map, &self.control_map))
                        .unwrap_or_else(|| {
                            self.turn_colors
                                .color_t(t)
                                .unwrap_or(render::TURN_ICON_INACTIVE_COLOR)
                        })
                })
        })
    }

    fn color_building(&self, id: map_model::BuildingID) -> Color {
        let b = self.map.get_b(id);
        vec![
            self.current_selection_state.color_b(b),
            self.current_search_state.color_b(b),
            if self.osm_classifier_active.is_enabled() {
                self.osm_classifier.color_b(b)
            } else {
                None
            },
        ].iter()
            .filter_map(|c| *c)
            .next()
            .unwrap_or(render::BUILDING_COLOR)
    }

    // Returns (boundary, fill) color
    fn color_parcel(&self, id: map_model::ParcelID) -> (Color, Color) {
        let _p = self.map.get_p(id);
        (render::PARCEL_BOUNDARY_COLOR, render::PARCEL_INTERIOR_COLOR)
    }

    fn color_car(&self, id: CarID) -> Color {
        if let Some(c) = self.current_selection_state.color_c(id) {
            return c;
        }
        if self.sim_ctrl.sim.is_moving(id) {
            canvas::CYAN
        } else {
            canvas::RED
        }
    }
}
