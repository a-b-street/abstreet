mod chokepoints;
mod color_picker;
mod connected_roads;
mod neighborhood_summary;
mod objects;
mod polygons;

use crate::common::CommonState;
use crate::game::{GameState, Mode};
use crate::objects::ID;
use crate::ui::{ShowLayers, ShowObject, UI};
use abstutil::Timer;
use ezgui::{
    Color, EventCtx, EventLoopMode, GfxCtx, InputResult, Key, ScrollingMenu, Text, TextBox, Wizard,
};
use map_model::RoadID;
use std::collections::{HashMap, HashSet};

pub struct DebugMode {
    state: State,
    common: CommonState,
    chokepoints: Option<chokepoints::ChokepointsFinder>,
    show_original_roads: HashSet<RoadID>,
    connected_roads: connected_roads::ShowConnectedRoads,
    objects: objects::ObjectDebugger,
    hidden: HashSet<ID>,
    layers: ShowLayers,
    search_results: Option<(String, HashSet<ID>)>,
    neighborhood_summary: neighborhood_summary::NeighborhoodSummary,
}

enum State {
    Exploring,
    Polygons(polygons::PolygonDebugger),
    SearchOSM(TextBox),
    Colors(color_picker::ColorPicker),
}

impl DebugMode {
    pub fn new(ctx: &mut EventCtx, ui: &UI) -> DebugMode {
        DebugMode {
            state: State::Exploring,
            common: CommonState::new(),
            chokepoints: None,
            show_original_roads: HashSet::new(),
            connected_roads: connected_roads::ShowConnectedRoads::new(),
            objects: objects::ObjectDebugger::new(),
            hidden: HashSet::new(),
            layers: ShowLayers::new(),
            search_results: None,
            neighborhood_summary: neighborhood_summary::NeighborhoodSummary::new(
                &ui.primary.map,
                &ui.primary.draw_map,
                ctx.prerender,
                &mut Timer::new("set up DebugMode"),
            ),
        }
    }

    pub fn event(state: &mut GameState, ctx: &mut EventCtx) -> EventLoopMode {
        match state.mode {
            Mode::Debug(ref mut mode) => {
                match mode.state {
                    State::Exploring => {
                        ctx.canvas.handle_event(ctx.input);
                        state.ui.primary.current_selection =
                            state
                                .ui
                                .handle_mouseover(ctx, None, &state.ui.primary.sim, mode, true);
                        if let Some(evmode) = mode.common.event(ctx, &state.ui) {
                            return evmode;
                        }

                        let mut txt = Text::new();
                        txt.add_styled_line(
                            "Debug Mode".to_string(),
                            None,
                            Some(Color::BLUE),
                            None,
                        );
                        if mode.chokepoints.is_some() {
                            txt.add_line("Showing chokepoints".to_string());
                        }
                        if !mode.show_original_roads.is_empty() {
                            txt.add_line(format!(
                                "Showing {} original roads",
                                mode.show_original_roads.len()
                            ));
                        }
                        if !mode.hidden.is_empty() {
                            txt.add_line(format!("Hiding {} things", mode.hidden.len()));
                        }
                        if let Some((ref search, ref results)) = mode.search_results {
                            txt.add_line(format!(
                                "Search for {} has {} results",
                                search,
                                results.len()
                            ));
                        }
                        if mode.neighborhood_summary.active {
                            txt.add_line("Showing neighborhood summaries".to_string());
                        }
                        ctx.input
                            .set_mode_with_new_prompt("Debug Mode", txt, ctx.canvas);
                        if ctx.input.modal_action("quit") {
                            state.mode = Mode::SplashScreen(Wizard::new(), None);
                            return EventLoopMode::InputOnly;
                        }

                        if ctx.input.modal_action("show/hide chokepoints") {
                            if mode.chokepoints.is_some() {
                                mode.chokepoints = None;
                            } else {
                                // TODO Nothing will actually exist. ;)
                                mode.chokepoints = Some(chokepoints::ChokepointsFinder::new(
                                    &state.ui.primary.sim,
                                ));
                            }
                        }
                        if !mode.show_original_roads.is_empty() {
                            if ctx.input.modal_action("clear original roads shown") {
                                mode.show_original_roads.clear();
                            }
                        }
                        if !mode.hidden.is_empty() {
                            if ctx.input.modal_action("unhide everything") {
                                mode.hidden.clear();
                                // TODO recalculate current_selection
                            }
                        }
                        match state.ui.primary.current_selection {
                            Some(ID::Lane(_))
                            | Some(ID::Intersection(_))
                            | Some(ID::ExtraShape(_)) => {
                                let id = state.ui.primary.current_selection.unwrap();
                                if ctx
                                    .input
                                    .contextual_action(Key::H, &format!("hide {:?}", id))
                                {
                                    println!("Hiding {:?}", id);
                                    //*ctx.recalculate_current_selection = true;
                                    state.ui.primary.current_selection = None;
                                    mode.hidden.insert(id);
                                }
                            }
                            _ => {}
                        }

                        if let Some(ID::Lane(l)) = state.ui.primary.current_selection {
                            let id = state.ui.primary.map.get_l(l).parent;
                            if ctx.input.contextual_action(
                                Key::V,
                                &format!("show original geometry of {:?}", id),
                            ) {
                                mode.show_original_roads.insert(id);
                            }
                        }
                        mode.connected_roads.event(ctx, &state.ui);
                        mode.objects.event(ctx, &state.ui);
                        mode.neighborhood_summary.event(ctx, &state.ui);

                        if let Some(debugger) = polygons::PolygonDebugger::new(ctx, &state.ui) {
                            mode.state = State::Polygons(debugger);
                        }

                        // TODO recalc current selection...
                        if ctx.input.modal_action("show/hide buildings") {
                            mode.layers.show_buildings = !mode.layers.show_buildings;
                        } else if ctx.input.modal_action("show/hide intersections") {
                            mode.layers.show_intersections = !mode.layers.show_intersections;
                        } else if ctx.input.modal_action("show/hide lanes") {
                            mode.layers.show_lanes = !mode.layers.show_lanes;
                        } else if ctx.input.modal_action("show/hide areas") {
                            mode.layers.show_areas = !mode.layers.show_areas;
                        } else if ctx.input.modal_action("show/hide extra shapes") {
                            mode.layers.show_extra_shapes = !mode.layers.show_extra_shapes;
                        } else if ctx.input.modal_action("show/hide geometry debug mode") {
                            mode.layers.geom_debug_mode = !mode.layers.geom_debug_mode;
                        }

                        if ctx.input.modal_action("screenshot everything") {
                            let bounds = state.ui.primary.map.get_bounds();
                            assert!(bounds.min_x == 0.0 && bounds.min_y == 0.0);
                            return EventLoopMode::ScreenCaptureEverything {
                                dir: format!(
                                    "../data/screenshots/pending_{}",
                                    state.ui.primary.map.get_name()
                                ),
                                zoom: 3.0,
                                max_x: bounds.max_x,
                                max_y: bounds.max_y,
                            };
                        }

                        if mode.search_results.is_some() {
                            if ctx.input.modal_action("clear OSM search results") {
                                mode.search_results = None;
                            }
                        } else if ctx.input.modal_action("search OSM metadata") {
                            mode.state = State::SearchOSM(TextBox::new("Search for what?", None));
                        } else if ctx.input.modal_action("configure colors") {
                            mode.state = State::Colors(color_picker::ColorPicker::Choosing(
                                ScrollingMenu::new(
                                    "Pick a color to change",
                                    state.ui.cs.color_names(),
                                ),
                            ));
                        }

                        EventLoopMode::InputOnly
                    }
                    State::Polygons(ref mut debugger) => {
                        if debugger.event(ctx) {
                            mode.state = State::Exploring;
                        }
                        EventLoopMode::InputOnly
                    }
                    State::SearchOSM(ref mut tb) => {
                        match tb.event(&mut ctx.input) {
                            InputResult::Canceled => {
                                mode.state = State::Exploring;
                            }
                            InputResult::Done(filter, _) => {
                                mode.state = State::Exploring;

                                let mut ids = HashSet::new();
                                let map = &state.ui.primary.map;
                                for r in map.all_roads() {
                                    if r.osm_tags
                                        .iter()
                                        .any(|(k, v)| format!("{} = {}", k, v).contains(&filter))
                                    {
                                        for l in r.all_lanes() {
                                            ids.insert(ID::Lane(l));
                                        }
                                    }
                                }
                                for b in map.all_buildings() {
                                    if b.osm_tags
                                        .iter()
                                        .any(|(k, v)| format!("{} = {}", k, v).contains(&filter))
                                    {
                                        ids.insert(ID::Building(b.id));
                                    }
                                }
                                mode.search_results = Some((filter, ids));
                            }
                            InputResult::StillActive => {}
                        }
                        EventLoopMode::InputOnly
                    }
                    State::Colors(ref mut picker) => {
                        if picker.event(ctx, &mut state.ui) {
                            mode.state = State::Exploring;
                        }
                        EventLoopMode::InputOnly
                    }
                }
            }
            _ => unreachable!(),
        }
    }

    pub fn draw(state: &GameState, g: &mut GfxCtx) {
        match state.mode {
            Mode::Debug(ref mode) => match mode.state {
                State::Exploring => {
                    let mut color_overrides = mode.common.override_colors(&state.ui);
                    if let Some(ref chokepoints) = mode.chokepoints {
                        let color = state.ui.cs.get_def("chokepoint", Color::RED);
                        for l in &chokepoints.lanes {
                            color_overrides.insert(ID::Lane(*l), color);
                        }
                        for i in &chokepoints.intersections {
                            color_overrides.insert(ID::Intersection(*i), color);
                        }
                    }
                    for l in &mode.connected_roads.lanes {
                        color_overrides.insert(
                            ID::Lane(*l),
                            state.ui.cs.get("something associated with something else"),
                        );
                    }
                    if let Some((_, ref results)) = mode.search_results {
                        for id in results {
                            color_overrides
                                .insert(*id, state.ui.cs.get_def("search result", Color::RED));
                        }
                    }
                    state
                        .ui
                        .new_draw(g, None, color_overrides, &state.ui.primary.sim, mode);
                    mode.common.draw(g, &state.ui);

                    for id in &mode.show_original_roads {
                        let r = state.ui.primary.map.get_r(*id);
                        if let Some(pair) = r.get_center_for_side(true) {
                            let (pl, width) = pair.unwrap();
                            g.draw_polygon(
                                state
                                    .ui
                                    .cs
                                    .get_def("original road forwards", Color::RED.alpha(0.5)),
                                &pl.make_polygons(width),
                            );
                        }
                        if let Some(pair) = r.get_center_for_side(false) {
                            let (pl, width) = pair.unwrap();
                            g.draw_polygon(
                                state
                                    .ui
                                    .cs
                                    .get_def("original road backwards", Color::BLUE.alpha(0.5)),
                                &pl.make_polygons(width),
                            );
                        }
                    }

                    mode.objects.draw(g, &state.ui);
                    mode.neighborhood_summary.draw(g);
                }
                State::Polygons(ref debugger) => {
                    state
                        .ui
                        .new_draw(g, None, HashMap::new(), &state.ui.primary.sim, mode);
                    debugger.draw(g, &state.ui);
                }
                State::SearchOSM(ref tb) => {
                    state
                        .ui
                        .new_draw(g, None, HashMap::new(), &state.ui.primary.sim, mode);
                    tb.draw(g);
                }
                State::Colors(ref picker) => {
                    state
                        .ui
                        .new_draw(g, None, HashMap::new(), &state.ui.primary.sim, mode);
                    picker.draw(g);
                }
            },
            _ => unreachable!(),
        }
    }
}

impl ShowObject for DebugMode {
    fn show(&self, obj: ID) -> bool {
        if self.hidden.contains(&obj) {
            return false;
        }

        match obj {
            ID::Road(_) | ID::Lane(_) => self.layers.show_lanes,
            ID::Building(_) => self.layers.show_buildings,
            ID::Intersection(_) => self.layers.show_intersections,
            ID::ExtraShape(_) => self.layers.show_extra_shapes,
            ID::Area(_) => self.layers.show_areas,
            _ => true,
        }
    }

    fn layers(&self) -> &ShowLayers {
        &self.layers
    }
}
