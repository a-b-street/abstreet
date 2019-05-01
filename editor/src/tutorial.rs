use crate::game::{GameState, Mode};
use crate::render::DrawOptions;
use crate::ui::ShowEverything;
use ezgui::{Color, EventCtx, EventLoopMode, GfxCtx, Key, Text, Wizard};
use geom::Pt2D;

// Does CommonState make sense?
pub enum TutorialMode {
    Part1(Pt2D),
    Part2(f64),
}

impl TutorialMode {
    pub fn event(state: &mut GameState, ctx: &mut EventCtx) -> EventLoopMode {
        ctx.canvas.handle_event(ctx.input);

        let mut txt = Text::new();
        txt.add_styled_line("Tutorial".to_string(), None, Some(Color::BLUE), None);
        match state.mode {
            Mode::Tutorial(TutorialMode::Part1(orig_center)) => {
                txt.add_line("Click and drag to pan around".to_string());

                // TODO Zooming also changes this. :(
                if ctx.canvas.center_to_map_pt() != orig_center {
                    txt.add_line("".to_string());
                    txt.add_line("Great! Press ENTER to continue.".to_string());
                    if ctx.input.key_pressed(Key::Enter, "next step of tutorial") {
                        state.mode = Mode::Tutorial(TutorialMode::Part2(ctx.canvas.cam_zoom));
                    }
                }
            }
            Mode::Tutorial(TutorialMode::Part2(orig_cam_zoom)) => {
                txt.add_line("Use your mouse wheel or touchpad to zoom in and out".to_string());

                if ctx.canvas.cam_zoom != orig_cam_zoom {
                    txt.add_line("".to_string());
                    txt.add_line("Great! Press ENTER to continue.".to_string());
                    if ctx.input.key_pressed(Key::Enter, "next step of tutorial") {
                        state.mode = Mode::SplashScreen(Wizard::new(), None);
                    }
                }
            }
            _ => unreachable!(),
        }
        ctx.input
            .set_mode_with_new_prompt("Tutorial", txt, ctx.canvas);

        if ctx.input.modal_action("quit") {
            state.mode = Mode::SplashScreen(Wizard::new(), None);
        }

        EventLoopMode::InputOnly
    }

    pub fn draw(state: &GameState, g: &mut GfxCtx) {
        state.ui.draw(
            g,
            DrawOptions::new(),
            &state.ui.primary.sim,
            &ShowEverything::new(),
        );
    }
}
