mod associated;
mod turn_cycler;

use crate::objects::ID;
use crate::ui::UI;
use ezgui::{Color, EventCtx, EventLoopMode, GfxCtx};
use std::collections::HashMap;

pub struct CommonState {
    associated: associated::ShowAssociatedState,
    turn_cycler: turn_cycler::TurnCyclerState,
}

impl CommonState {
    pub fn new() -> CommonState {
        CommonState {
            associated: associated::ShowAssociatedState::Inactive,
            turn_cycler: turn_cycler::TurnCyclerState::new(),
        }
    }

    // If this returns something, then this common state should prevent other things from
    // happening.
    pub fn event(&mut self, ctx: &mut EventCtx, ui: &UI) -> Option<EventLoopMode> {
        self.associated.event(ui);
        self.turn_cycler.event(ctx, ui);
        None
    }

    pub fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        self.turn_cycler.draw(g, ui);
    }

    pub fn override_colors(&self, ui: &UI) -> HashMap<ID, Color> {
        let mut colors = HashMap::new();
        self.associated.override_colors(&mut colors, ui);
        colors
    }
}
