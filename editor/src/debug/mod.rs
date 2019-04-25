mod chokepoints;
mod polygons;

use crate::game::{GameState, Mode};
use crate::objects::ID;
use ezgui::{Color, EventCtx, EventLoopMode, GfxCtx, Key, Text, Wizard};
use map_model::RoadID;
use std::collections::{HashMap, HashSet};

pub struct DebugMode {
    state: State,
    chokepoints: Option<chokepoints::ChokepointsFinder>,
    show_original_roads: HashSet<RoadID>,
}

enum State {
    Exploring,
    Polygons(polygons::PolygonDebugger),
}

impl DebugMode {
    pub fn new() -> DebugMode {
        DebugMode {
            state: State::Exploring,
            chokepoints: None,
            show_original_roads: HashSet::new(),
        }
    }

    pub fn event(state: &mut GameState, ctx: &mut EventCtx) -> EventLoopMode {
        match state.mode {
            Mode::Debug(ref mut mode) => {
                match mode.state {
                    State::Exploring => {
                        ctx.canvas.handle_event(ctx.input);
                        state.ui.state.primary.current_selection =
                            state
                                .ui
                                .handle_mouseover(ctx, None, &state.ui.state.primary.sim);

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
                                    &state.ui.state.primary.sim,
                                ));
                            }
                        }
                        if !mode.show_original_roads.is_empty() {
                            if ctx.input.modal_action("clear original roads shown") {
                                mode.show_original_roads.clear();
                            }
                        }

                        if let Some(ID::Lane(l)) = state.ui.state.primary.current_selection {
                            let id = state.ui.state.primary.map.get_l(l).parent;
                            if ctx.input.contextual_action(
                                Key::V,
                                &format!("show original geometry of {:?}", id),
                            ) {
                                mode.show_original_roads.insert(id);
                            }
                        }

                        if let Some(debugger) = polygons::PolygonDebugger::new(ctx, &state.ui) {
                            mode.state = State::Polygons(debugger);
                        }

                        EventLoopMode::InputOnly
                    }
                    State::Polygons(ref mut debugger) => {
                        if debugger.event(ctx) {
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
                    let mut color_overrides = HashMap::new();
                    if let Some(ref chokepoints) = mode.chokepoints {
                        let color = state.ui.state.cs.get_def("chokepoint", Color::RED);
                        for l in &chokepoints.lanes {
                            color_overrides.insert(ID::Lane(*l), color);
                        }
                        for i in &chokepoints.intersections {
                            color_overrides.insert(ID::Intersection(*i), color);
                        }
                    }
                    state
                        .ui
                        .new_draw(g, None, color_overrides, &state.ui.state.primary.sim);

                    for id in &mode.show_original_roads {
                        let r = state.ui.state.primary.map.get_r(*id);
                        if let Some(pair) = r.get_center_for_side(true) {
                            let (pl, width) = pair.unwrap();
                            g.draw_polygon(
                                state
                                    .ui
                                    .state
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
                                    .state
                                    .cs
                                    .get_def("original road backwards", Color::BLUE.alpha(0.5)),
                                &pl.make_polygons(width),
                            );
                        }
                    }
                }
                State::Polygons(ref debugger) => {
                    state
                        .ui
                        .new_draw(g, None, HashMap::new(), &state.ui.state.primary.sim);
                    debugger.draw(g, &state.ui);
                }
            },
            _ => unreachable!(),
        }
    }
}
