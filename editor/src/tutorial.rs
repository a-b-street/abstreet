use crate::game::{GameState, Mode};
use crate::render::DrawOptions;
use crate::ui::ShowEverything;
use ezgui::{EventCtx, EventLoopMode, GfxCtx, Key, ModalMenu, Text, Wizard};
use geom::Pt2D;

pub struct TutorialMode {
    menu: ModalMenu,
    // TODO Does CommonState make sense?
    state: State,
}

enum State {
    Part1(Pt2D),
    Part2(f64),
}

impl TutorialMode {
    pub fn new(ctx: &EventCtx) -> TutorialMode {
        TutorialMode {
            menu: ModalMenu::new("Tutorial", vec![(Some(Key::Escape), "quit")], ctx),
            state: State::Part1(ctx.canvas.center_to_map_pt()),
        }
    }

    pub fn event(state: &mut GameState, ctx: &mut EventCtx) -> EventLoopMode {
        match state.mode {
            Mode::Tutorial(ref mut mode) => {
                let mut txt = Text::prompt("Tutorial");
                match mode.state {
                    State::Part1(orig_center) => {
                        txt.add_line("Click and drag to pan around".to_string());

                        // TODO Zooming also changes this. :(
                        if ctx.canvas.center_to_map_pt() != orig_center {
                            txt.add_line("".to_string());
                            txt.add_line("Great! Press ENTER to continue.".to_string());
                            if ctx.input.key_pressed(Key::Enter, "next step of tutorial") {
                                mode.state = State::Part2(ctx.canvas.cam_zoom);
                            }
                        }
                    }
                    State::Part2(orig_cam_zoom) => {
                        txt.add_line(
                            "Use your mouse wheel or touchpad to zoom in and out".to_string(),
                        );

                        if ctx.canvas.cam_zoom != orig_cam_zoom {
                            txt.add_line("".to_string());
                            txt.add_line("Great! Press ENTER to continue.".to_string());
                            if ctx.input.key_pressed(Key::Enter, "next step of tutorial") {
                                state.mode = Mode::SplashScreen(Wizard::new(), None);
                                return EventLoopMode::InputOnly;
                            }
                        }
                    }
                }
                mode.menu.handle_event(ctx, Some(txt));
                ctx.canvas.handle_event(ctx.input);

                if mode.menu.action("quit") {
                    state.mode = Mode::SplashScreen(Wizard::new(), None);
                }

                EventLoopMode::InputOnly
            }
            _ => unreachable!(),
        }
    }

    pub fn draw(state: &GameState, g: &mut GfxCtx) {
        state.ui.draw(
            g,
            DrawOptions::new(),
            &state.ui.primary.sim,
            &ShowEverything::new(),
        );
        match state.mode {
            Mode::Tutorial(ref mode) => {
                mode.menu.draw(g);
            }
            _ => unreachable!(),
        }
    }
}
