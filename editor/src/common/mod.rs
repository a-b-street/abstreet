mod associated;
mod turn_cycler;
mod warp;

use crate::objects::ID;
use crate::ui::UI;
use ezgui::{Color, EventCtx, EventLoopMode, GfxCtx, Key};
use std::collections::HashMap;

pub struct CommonState {
    associated: associated::ShowAssociatedState,
    turn_cycler: turn_cycler::TurnCyclerState,
    warp: Option<warp::WarpState>,
}

impl CommonState {
    pub fn new() -> CommonState {
        CommonState {
            associated: associated::ShowAssociatedState::Inactive,
            turn_cycler: turn_cycler::TurnCyclerState::new(),
            warp: None,
        }
    }

    // If this returns something, then this common state should prevent other things from
    // happening.
    pub fn event(&mut self, ctx: &mut EventCtx, ui: &UI) -> Option<EventLoopMode> {
        if let Some(ref mut warp) = self.warp {
            if let Some(evmode) = warp.event(ctx, ui) {
                return Some(evmode);
            }
            self.warp = None;
        }
        if ctx.input.unimportant_key_pressed(Key::J, "warp") {
            self.warp = Some(warp::WarpState::new());
        }

        self.associated.event(ui);
        self.turn_cycler.event(ctx, ui);
        // TODO How to reserve and explain this key?
        if ctx
            .input
            .unimportant_key_pressed(Key::F1, "screenshot just this")
        {
            return Some(EventLoopMode::ScreenCaptureCurrentShot);
        }
        None
    }

    pub fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        if let Some(ref warp) = self.warp {
            warp.draw(g);
        }
        self.turn_cycler.draw(g, ui);
    }

    pub fn override_colors(&self, ui: &UI) -> HashMap<ID, Color> {
        let mut colors = HashMap::new();
        self.associated.override_colors(&mut colors, ui);
        colors
    }
}
