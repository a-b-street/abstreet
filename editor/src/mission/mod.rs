use crate::game::{GameState, Mode};
use crate::ui::ShowEverything;
use ezgui::{EventCtx, EventLoopMode, GfxCtx, Wizard};
use std::collections::HashMap;

pub struct MissionEditMode {
    state: State,
}

enum State {
    Exploring,
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
                ctx.canvas.handle_event(ctx.input);
                state.ui.state.primary.current_selection = state.ui.handle_mouseover(
                    ctx,
                    None,
                    &state.ui.state.primary.sim,
                    &ShowEverything::new(),
                );

                ctx.input.set_mode("Mission Edit Mode", ctx.canvas);
                if ctx.input.modal_action("quit") {
                    state.mode = Mode::SplashScreen(Wizard::new(), None);
                }

                EventLoopMode::InputOnly
            }
            _ => unreachable!(),
        }
    }

    pub fn draw(state: &GameState, g: &mut GfxCtx) {
        match state.mode {
            Mode::Mission(ref mode) => match mode.state {
                State::Exploring => {
                    state.ui.new_draw(
                        g,
                        None,
                        HashMap::new(),
                        &state.ui.state.primary.sim,
                        &ShowEverything::new(),
                    );
                }
            },
            _ => unreachable!(),
        }
    }
}
