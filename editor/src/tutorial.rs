use crate::game::{GameState, Mode};
use crate::render::DrawOptions;
use crate::ui::ShowEverything;
use ezgui::{
    EventCtx, EventLoopMode, GfxCtx, HorizontalAlignment, Key, Text, VerticalAlignment, Wizard,
};
use geom::Pt2D;

// Does CommonState make sense?
pub enum TutorialMode {
    Part1(Pt2D),
    Part2(f64),
}

impl TutorialMode {
    pub fn event(state: &mut GameState, ctx: &mut EventCtx) -> EventLoopMode {
        ctx.canvas.handle_event(ctx.input);

        match state.mode {
            Mode::Tutorial(TutorialMode::Part1(orig_center)) => {
                // TODO Zooming also changes this. :(
                if ctx.canvas.center_to_map_pt() != orig_center
                    && ctx.input.key_pressed(Key::Enter, "next step of tutorial")
                {
                    state.mode = Mode::Tutorial(TutorialMode::Part2(ctx.canvas.cam_zoom));
                }
            }
            Mode::Tutorial(TutorialMode::Part2(orig_cam_zoom)) => {
                if ctx.canvas.cam_zoom != orig_cam_zoom
                    && ctx.input.key_pressed(Key::Enter, "next step of tutorial")
                {
                    state.mode = Mode::SplashScreen(Wizard::new(), None);
                }
            }
            _ => unreachable!(),
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

        let mut txt = Text::new();
        match state.mode {
            Mode::Tutorial(TutorialMode::Part1(orig_center)) => {
                txt.add_line("Click and drag to pan around".to_string());
                if g.canvas.center_to_map_pt() != orig_center {
                    txt.add_line("".to_string());
                    txt.add_line("Great! Press ENTER to continue.".to_string());
                }
            }
            Mode::Tutorial(TutorialMode::Part2(orig_cam_zoom)) => {
                txt.add_line("Use your mouse wheel or touchpad to zoom in and out".to_string());
                if g.canvas.cam_zoom != orig_cam_zoom {
                    txt.add_line("".to_string());
                    txt.add_line("Great! Press ENTER to continue.".to_string());
                }
            }
            _ => unreachable!(),
        }
        // TODO Get rid of top menu and OSD and then put this somewhere more sensible. :)
        g.draw_blocking_text(
            &txt,
            (HorizontalAlignment::Right, VerticalAlignment::Center),
        );
    }
}
