use crate::objects::Ctx;
use crate::plugins::{Plugin, PluginCtx};
use ezgui::{GfxCtx, LogScroller};

pub enum TutorialMode {
    GiveInstructions(LogScroller),
    Play,
}

impl TutorialMode {
    pub fn new() -> TutorialMode {
        TutorialMode::GiveInstructions(LogScroller::new_from_lines(vec![
            "Welcome to the A/B Street tutorial!".to_string(),
            "".to_string(),
            "There'll be some instructions here eventually. Fix all the traffic!".to_string(),
        ]))
    }
}

impl Plugin for TutorialMode {
    fn blocking_event(&mut self, ctx: &mut PluginCtx) -> bool {
        match self {
            TutorialMode::GiveInstructions(ref mut scroller) => {
                if scroller.event(&mut ctx.input) {
                    *self = TutorialMode::Play;
                }
                true
            }
            TutorialMode::Play => false,
        }
    }

    fn draw(&self, g: &mut GfxCtx, ctx: &mut Ctx) {
        match self {
            TutorialMode::GiveInstructions(ref scroller) => {
                scroller.draw(g, ctx.canvas);
            }
            TutorialMode::Play => {}
        };
    }
}
