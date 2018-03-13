// Copyright 2018 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

// TODO this should just be a way to handle interactions between plugins

extern crate map_model;

use animation;
use ezgui::ToggleableLayer;
use ezgui::canvas::{Canvas, GfxCtx};
use graphics::types::Color;
use control::ControlMap;
use ezgui::input::UserInput;
use piston::input::{Key, MouseCursorEvent, UpdateEvent};
use piston::window::Size;
use plugins::classification::OsmClassifier;
use plugins::floodfill::Floodfiller;
use plugins::search::SearchState;
use plugins::selection::{SelectionState, ID};
use plugins::snake::Snake;
use plugins::steep::SteepnessVisualizer;
use plugins::stop_sign_editor::StopSignEditor;
use plugins::traffic_signal_editor::TrafficSignalEditor;
use plugins::turn_colors::TurnColors;
use render;
use render::ColorChooser;
use savestate;
use sim::straw_model::Sim;
use std::io;
use std::process;
use svg;

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
    // TODO should these be more associated with their plugins?
    steepness_active: ToggleableLayer,
    osm_classifier_active: ToggleableLayer,
    debug_mode: ToggleableLayer,
    // TODO weird to be here...
    sim_running: ToggleableLayer,
    sim_time: animation::SimTime,

    current_selection_state: SelectionState,
    current_search_state: SearchState,
    floodfiller: Option<Floodfiller>,
    snake: Option<Snake>,
    steepness_viz: SteepnessVisualizer,
    osm_classifier: OsmClassifier,
    turn_colors: TurnColors,
    traffic_signal_editor: Option<TrafficSignalEditor>,
    stop_sign_editor: Option<StopSignEditor>,
    sim: Sim,

    canvas: Canvas,
    max_screen_pt: map_model::Pt2D,
}

impl UI {
    pub fn new(osm_path: &str, window_size: &Size) -> UI {
        println!("Opening {}", osm_path);
        let data = map_model::load_pb(osm_path).expect("Couldn't load pb");
        let map = map_model::Map::new(&data);
        let (draw_map, _, center_pt, max_screen_pt) = render::DrawMap::new(&map);
        let control_map = ControlMap::new(&map, &draw_map);

        let steepness_viz = SteepnessVisualizer::new(&map);
        let turn_colors = TurnColors::new(&control_map);
        let sim = Sim::new(&map, &draw_map);

        let mut ui = UI {
            map,
            draw_map,
            control_map,
            steepness_viz,
            turn_colors,
            max_screen_pt,
            sim,

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
                "road and turn icons",
                Key::D7,
                "7",
                Some(MIN_ZOOM_FOR_ROAD_MARKERS),
            ),
            steepness_active: ToggleableLayer::new("steepness visualize", Key::D5, "5", None),
            osm_classifier_active: ToggleableLayer::new("OSM type classifier", Key::D6, "6", None),
            debug_mode: ToggleableLayer::new("debug mode", Key::D, "D", None),
            sim_running: ToggleableLayer::new("sim", Key::Space, "Space", None),
            sim_time: animation::SimTime::new(),

            current_selection_state: SelectionState::Empty,
            current_search_state: SearchState::Empty,
            floodfiller: None,
            snake: None,
            osm_classifier: OsmClassifier {},
            traffic_signal_editor: None,
            stop_sign_editor: None,

            canvas: Canvas::new(),
        };

        match savestate::load("editor_state") {
            Ok(state) => {
                println!("Loaded previous editor_state");
                ui.canvas.cam_x = state.cam_x;
                ui.canvas.cam_y = state.cam_y;
                ui.canvas.cam_zoom = state.cam_zoom;
                ui.control_map.load_savestate(&state);
            }
            Err(_) => {
                println!("Couldn't load editor_state, just centering initial view");
                ui.canvas
                    .center_on_map_pt(center_pt.x(), center_pt.y(), window_size);
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
        ui.sim_running.handle_zoom(old_zoom, new_zoom);

        ui
    }

    pub fn event(
        mut self,
        input: &mut UserInput,
        window_size: &Size,
    ) -> (UI, animation::EventLoopMode) {
        if let Some(mut s) = self.snake {
            if s.event(
                input,
                &self.map,
                &self.draw_map,
                &mut self.canvas,
                window_size,
            ) {
                self.snake = None;
            } else {
                self.snake = Some(s);
                return (self, animation::EventLoopMode::Animation);
            }
        }

        let mut event_loop_mode = animation::EventLoopMode::InputOnly;

        if self.sim_running.is_enabled() {
            if input.use_event_directly().update_args().is_some() {
                self.sim
                    .step(self.sim_time.get_dt_s(), &self.draw_map, &self.map);
            }
            event_loop_mode = event_loop_mode.merge(animation::EventLoopMode::Animation);
        }

        if let Some(mut e) = self.traffic_signal_editor {
            if e.event(
                input,
                &self.map,
                &self.draw_map,
                &mut self.control_map,
                &self.current_selection_state,
            ) {
                self.traffic_signal_editor = None;
            } else {
                self.traffic_signal_editor = Some(e);
            }
        }

        if let Some(mut e) = self.stop_sign_editor {
            if e.event(
                input,
                &self.map,
                &self.draw_map,
                &mut self.control_map,
                &self.current_selection_state,
            ) {
                self.stop_sign_editor = None;
            } else {
                self.stop_sign_editor = Some(e);
            }
        }

        self.current_search_state = self.current_search_state.event(input);

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
        self.sim_running.handle_zoom(old_zoom, new_zoom);

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
            if let SelectionState::SelectedIntersection(_, _, _) = self.current_selection_state {
                self.current_selection_state = SelectionState::Empty;
            }
        }
        self.show_parcels.handle_event(input);
        self.show_icons.handle_event(input);
        self.steepness_active.handle_event(input);
        self.osm_classifier_active.handle_event(input);
        self.debug_mode.handle_event(input);
        self.sim_running.handle_event(input);
        // TODO this feels like a hack
        self.sim_time.set_active(self.sim_running.is_enabled());

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
        let (new_selection_state, new_event_loop_mode) = self.current_selection_state.event(input);
        event_loop_mode = event_loop_mode.merge(new_event_loop_mode);
        self.current_selection_state = new_selection_state;
        match self.current_selection_state {
            SelectionState::SelectedRoad(id, _) => {
                if self.floodfiller.is_none() {
                    if input.key_pressed(Key::F, "Press F to start floodfilling from this road") {
                        self.floodfiller = Some(Floodfiller::new(id));
                    }
                }

                if self.snake.is_none() {
                    if input.key_pressed(Key::S, "Press S to start a game of Snake from this road")
                    {
                        self.snake = Some(Snake::new(id));
                        // TODO weird to reset other things here?
                        self.current_selection_state = SelectionState::Empty;
                    }
                }

                if !self.sim_running.is_enabled() {
                    if input.key_pressed(Key::A, "Press A to add a car starting from this road") {
                        self.sim.spawn_one_on_road(id);
                    }
                }
            }
            SelectionState::SelectedIntersection(id, _, _) => {
                if self.traffic_signal_editor.is_none()
                    && self.control_map.traffic_signals.contains_key(&id)
                {
                    if input.key_pressed(Key::E, "Press E to edit this traffic signal") {
                        self.traffic_signal_editor = Some(TrafficSignalEditor::new(id));
                    }
                }
                if self.stop_sign_editor.is_none() && self.control_map.stop_signs.contains_key(&id)
                {
                    if input.key_pressed(Key::E, "Press E to edit this stop sign ") {
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

        if input.unimportant_key_pressed(Key::S, "Spawn 100 cars in random places") {
            self.sim.spawn_many_on_empty_roads(100);
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
            savestate::write("editor_state", state).expect("Saving editor state failed");
            println!("Saved editor_state");
            process::exit(0);
        }

        (self, event_loop_mode)
    }

    // TODO it'd be neat if all of the Draw*'s didn't have to know how to separately emit svg
    pub fn save_svg(&self, path: &str) -> Result<(), io::Error> {
        let mut doc = svg::Document::new();
        doc = doc.set(
            "viewBox",
            (0, 0, self.max_screen_pt.x(), self.max_screen_pt.y()),
        );
        for r in &self.draw_map.roads {
            doc = r.to_svg(
                doc,
                self.color_road(self.map.get_r(r.id)),
                self.color_road_icon(self.map.get_r(r.id)),
            );
        }
        for i in &self.draw_map.intersections {
            doc = i.to_svg(doc, self.color_intersection(self.map.get_i(i.id)));
        }
        for t in &self.draw_map.turns {
            doc = t.to_svg(doc, self.color_turn_icon(self.map.get_t(t.id)));
        }
        for b in &self.draw_map.buildings {
            doc = b.to_svg(doc, self.color_building(self.map.get_b(b.id)));
        }
        for p in &self.draw_map.parcels {
            doc = p.to_svg(doc, self.color_parcel(self.map.get_p(p.id)));
        }
        println!("Dumping SVG to {}", path);
        svg::save(path, &doc)
    }

    pub fn draw(&self, g: &mut GfxCtx, input: UserInput) {
        g.ctx = self.canvas.get_transformed_context(&g.orig_ctx);

        let screen_bbox = self.canvas.get_screen_bbox(&g.window_size);

        if self.show_roads.is_enabled() {
            for r in &self.draw_map.get_roads_onscreen(screen_bbox) {
                r.draw(g, self.color_road(self.map.get_r(r.id)));
                if self.canvas.cam_zoom >= MIN_ZOOM_FOR_ROAD_MARKERS {
                    r.draw_detail(g);
                }
                if self.debug_mode.is_enabled() {
                    r.draw_debug(g);
                }
                self.sim.draw_cars_on_road(r.id, &self.draw_map, g);
            }
        }

        if self.show_intersections.is_enabled() {
            for i in &self.draw_map.get_intersections_onscreen(screen_bbox) {
                i.draw(g, self.color_intersection(self.map.get_i(i.id)));
            }
        }

        if self.show_icons.is_enabled() {
            for t in &self.draw_map.get_turn_icons_onscreen(screen_bbox) {
                t.draw_icon(g, self.color_turn_icon(self.map.get_t(t.id)));
                self.sim.draw_cars_on_turn(t.id, &self.draw_map, g);
            }
            for r in &self.draw_map.get_road_icons_onscreen(screen_bbox) {
                r.draw_icon(g, self.color_road_icon(self.map.get_r(r.id)));
            }
        }

        if self.show_parcels.is_enabled() {
            for p in &self.draw_map.get_parcels_onscreen(screen_bbox) {
                p.draw(g, self.color_parcel(self.map.get_p(p.id)));
            }
        }

        if self.show_buildings.is_enabled() {
            for b in &self.draw_map.get_buildings_onscreen(screen_bbox) {
                b.draw(g, self.color_building(self.map.get_b(b.id)));
            }
        }

        self.current_selection_state.draw(
            &self.map,
            &self.canvas,
            &self.draw_map,
            &self.control_map,
            g,
        );

        self.current_search_state.draw(&self.canvas, g);

        if let Some(ref s) = self.snake {
            s.draw(&self.map, &self.canvas, &self.draw_map, g);
        }

        self.sim.draw(&self.canvas, g);

        self.canvas
            .draw_osd_notification(g, &input.get_possible_actions());
    }

    fn mouseover_something(&self, window_size: &Size) -> Option<ID> {
        let (x, y) = self.canvas.get_cursor_in_map_space();

        let screen_bbox = self.canvas.get_screen_bbox(window_size);

        if self.show_icons.is_enabled() {
            for t in &self.draw_map.get_turn_icons_onscreen(screen_bbox) {
                if t.contains_pt(x, y) {
                    return Some(ID::Turn(t.id));
                }
            }

            for r in &self.draw_map.get_road_icons_onscreen(screen_bbox) {
                if r.icon_contains_pt(x, y) {
                    return Some(ID::RoadIcon(r.id));
                }
            }
        }

        if self.show_intersections.is_enabled() {
            for i in &self.draw_map.get_intersections_onscreen(screen_bbox) {
                if i.contains_pt(x, y) {
                    return Some(ID::Intersection(i.id));
                }
            }
        }

        if self.show_roads.is_enabled() {
            for r in &self.draw_map.get_roads_onscreen(screen_bbox) {
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

    fn color_road(&self, r: &map_model::Road) -> Color {
        // TODO This evaluates all the color methods, which may be expensive. But the option
        // chaining is harder to read. :(
        vec![
            self.current_selection_state.color_r(r),
            self.current_search_state.color_r(r),
            self.floodfiller.as_ref().and_then(|f| f.color_r(r)),
            self.snake.as_ref().and_then(|s| s.color_r(r)),
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
            .unwrap_or(render::ROAD_COLOR)
    }

    fn color_intersection(&self, i: &map_model::Intersection) -> Color {
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

        self.current_selection_state.color_i(i).unwrap_or_else(|| {
            self.current_search_state
                .color_i(i)
                .unwrap_or(default_color)
        })
    }

    fn color_turn_icon(&self, t: &map_model::Turn) -> Color {
        // TODO traffic signal selection logic maybe moves here
        self.current_selection_state.color_t(t).unwrap_or_else(|| {
            self.traffic_signal_editor
                .as_ref()
                .and_then(|e| e.color_t(t, &self.draw_map, &self.control_map))
                .unwrap_or_else(|| {
                    self.turn_colors
                        .color_t(t)
                        .unwrap_or(render::TURN_ICON_INACTIVE_COLOR)
                })
        })
    }

    // TODO rename these stop signs yo
    fn color_road_icon(&self, r: &map_model::Road) -> Color {
        // TODO color logic is leaking everywhere :(
        if let Some(c) = self.current_selection_state.color_road_icon(r) {
            return c;
        }
        // TODO ask the editor
        if let Some(s) = self.control_map
            .stop_signs
            .get(&self.map.get_destination_intersection(r.id).id)
        {
            if s.is_priority_road(r.id) {
                return render::NEXT_QUEUED_COLOR;
            }
        }

        render::QUEUED_COLOR
    }

    fn color_building(&self, b: &map_model::Building) -> Color {
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

    fn color_parcel(&self, _: &map_model::Parcel) -> Color {
        render::PARCEL_COLOR
    }
}
