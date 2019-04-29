mod neighborhood;
mod scenario;

use crate::game::{GameState, Mode};
use crate::ui::ShowEverything;
use ezgui::{EventCtx, EventLoopMode, GfxCtx, Wizard};
use std::collections::HashMap;

pub struct MissionEditMode {
    state: State,
}

enum State {
    Exploring,
    Neighborhood(neighborhood::NeighborhoodEditor),
    Scenario(scenario::ScenarioEditor),
}

impl MissionEditMode {
    pub fn new() -> MissionEditMode {
        MissionEditMode {
            state: State::Exploring,
        }
    }

    pub fn event(state: &mut GameState, ctx: &mut EventCtx) -> EventLoopMode {
        match state.mode {
            Mode::Mission(ref mut mode) => {
                match mode.state {
                    State::Exploring => {
                        ctx.canvas.handle_event(ctx.input);
                        state.ui.primary.current_selection = state.ui.handle_mouseover(
                            ctx,
                            None,
                            &state.ui.primary.sim,
                            &ShowEverything::new(),
                            false,
                        );

                        ctx.input.set_mode("Mission Edit Mode", ctx.canvas);
                        if ctx.input.modal_action("quit") {
                            state.mode = Mode::SplashScreen(Wizard::new(), None);
                        } else if ctx.input.modal_action("manage neighborhoods") {
                            mode.state = State::Neighborhood(
                                neighborhood::NeighborhoodEditor::PickNeighborhood(Wizard::new()),
                            );
                        } else if ctx.input.modal_action("manage scenarios") {
                            mode.state = State::Scenario(scenario::ScenarioEditor::PickScenario(
                                Wizard::new(),
                            ));
                        }
                    }
                    State::Neighborhood(ref mut editor) => {
                        if editor.event(ctx, &state.ui) {
                            mode.state = State::Exploring;
                        }
                    }
                    State::Scenario(ref mut editor) => {
                        if let Some(new_mode) = editor.event(ctx, &mut state.ui) {
                            state.mode = new_mode;
                        }
                    }
                }
                EventLoopMode::InputOnly
            }
            _ => unreachable!(),
        }
    }

    pub fn draw(state: &GameState, g: &mut GfxCtx) {
        state.ui.new_draw(
            g,
            None,
            HashMap::new(),
            &state.ui.primary.sim,
            &ShowEverything::new(),
        );

        match state.mode {
            Mode::Mission(ref mode) => match mode.state {
                State::Exploring => {}
                State::Neighborhood(ref editor) => {
                    editor.draw(g, &state.ui);
                }
                State::Scenario(ref editor) => {
                    editor.draw(g, &state.ui);
                }
            },
            _ => unreachable!(),
        }
    }
}
