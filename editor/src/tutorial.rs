use crate::game::{GameState, Mode};
use ezgui::{
    EventCtx, EventLoopMode, GfxCtx, HorizontalAlignment, Key, Text, VerticalAlignment, Wizard, GUI,
};
use geom::Pt2D;

pub enum TutorialState {
    Part1(Pt2D),
    Part2(f64),
}

impl TutorialState {
    pub fn event(state: &mut GameState, ctx: EventCtx) -> EventLoopMode {
        match state.mode {
            Mode::Tutorial(TutorialState::Part1(orig_center)) => {
                // TODO Zooming also changes this. :(
                if ctx.canvas.center_to_map_pt() != orig_center
                    && ctx.input.key_pressed(Key::Enter, "next step of tutorial")
                {
                    state.mode = Mode::Tutorial(TutorialState::Part2(ctx.canvas.cam_zoom));
                }
                let (event_mode, pause) = state.ui.new_event(ctx);
                if pause {
                    state.mode = Mode::SplashScreen(Wizard::new());
                }
                event_mode
            }
            Mode::Tutorial(TutorialState::Part2(orig_cam_zoom)) => {
                if ctx.canvas.cam_zoom != orig_cam_zoom
                    && ctx.input.key_pressed(Key::Enter, "next step of tutorial")
                {
                    state.mode = Mode::SplashScreen(Wizard::new());
                }
                let (event_mode, pause) = state.ui.new_event(ctx);
                if pause {
                    state.mode = Mode::SplashScreen(Wizard::new());
                }
                event_mode
            }
            _ => unreachable!(),
        }
    }

    pub fn draw(state: &GameState, g: &mut GfxCtx) {
        match state.mode {
            Mode::Tutorial(TutorialState::Part1(orig_center)) => {
                state.ui.draw(g);
                let mut txt = Text::new();
                txt.add_line("Click and drag to pan around".to_string());
                if g.canvas.center_to_map_pt() != orig_center {
                    txt.add_line("".to_string());
                    txt.add_line("Great! Press ENTER to continue.".to_string());
                }
                // TODO Get rid of top menu and OSD and then put this somewhere more sensible. :)
                g.draw_blocking_text(
                    &txt,
                    (HorizontalAlignment::Right, VerticalAlignment::Center),
                );
            }
            Mode::Tutorial(TutorialState::Part2(orig_cam_zoom)) => {
                state.ui.draw(g);
                let mut txt = Text::new();
                txt.add_line("Use your mouse wheel or touchpad to zoom in and out".to_string());
                if g.canvas.cam_zoom != orig_cam_zoom {
                    txt.add_line("".to_string());
                    txt.add_line("Great! Press ENTER to continue.".to_string());
                }
                g.draw_blocking_text(
                    &txt,
                    (HorizontalAlignment::Right, VerticalAlignment::Center),
                );
            }
            _ => unreachable!(),
        }
    }
}
