mod chokepoints;

use crate::game::{GameState, Mode};
use crate::objects::ID;
use ezgui::{Color, EventCtx, EventLoopMode, GfxCtx, Text, Wizard};
use std::collections::HashMap;

pub struct DebugMode {
    state: State,
    chokepoints: Option<chokepoints::ChokepointsFinder>,
}

enum State {
    Exploring,
}

impl DebugMode {
    pub fn new() -> DebugMode {
        DebugMode {
            state: State::Exploring,
            chokepoints: None,
        }
    }

    pub fn event(state: &mut GameState, ctx: &mut EventCtx) -> EventLoopMode {
        match state.mode {
            Mode::Debug(ref mut mode) => {
                ctx.canvas.handle_event(ctx.input);
                state.ui.state.primary.current_selection =
                    state
                        .ui
                        .handle_mouseover(ctx, None, &state.ui.state.primary.sim);

                let mut txt = Text::new();
                txt.add_styled_line("Debug Mode".to_string(), None, Some(Color::BLUE), None);
                if mode.chokepoints.is_some() {
                    txt.add_line("Showing chokepoints".to_string());
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

                EventLoopMode::InputOnly
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
                }
            },
            _ => unreachable!(),
        }
    }
}
