mod associated;
mod navigate;
mod turn_cycler;
mod warp;

use crate::helpers::ID;
use crate::render::DrawOptions;
use crate::ui::UI;
use abstutil::elapsed_seconds;
use ezgui::{EventCtx, EventLoopMode, GfxCtx, Key};
use geom::{Line, Pt2D};
use std::time::Instant;

pub struct CommonState {
    associated: associated::ShowAssociatedState,
    turn_cycler: turn_cycler::TurnCyclerState,
    warp: Option<warp::WarpState>,
    navigate: Option<navigate::Navigator>,
}

impl CommonState {
    pub fn new() -> CommonState {
        CommonState {
            associated: associated::ShowAssociatedState::Inactive,
            turn_cycler: turn_cycler::TurnCyclerState::new(),
            warp: None,
            navigate: None,
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
        if let Some(ref mut navigate) = self.navigate {
            if let Some(evmode) = navigate.event(ctx, ui) {
                return Some(evmode);
            }
            self.navigate = None;
        }
        // TODO This definitely conflicts with some modes.
        if ctx.input.unimportant_key_pressed(Key::K, "navigate") {
            self.navigate = Some(navigate::Navigator::new(ui));
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
        if let Some(ref navigate) = self.navigate {
            navigate.draw(g);
        }
        self.turn_cycler.draw(g, ui);
    }

    pub fn draw_options(&self, ui: &UI) -> DrawOptions {
        let mut opts = DrawOptions::new();
        self.associated
            .override_colors(&mut opts.override_colors, ui);
        // On behalf of turn_cycler, just do this directly here.
        if let Some(ID::Lane(l)) = ui.primary.current_selection {
            opts.suppress_traffic_signal_details = Some(ui.primary.map.get_l(l).dst_i);
        }
        opts
    }
}

// TODO Maybe pixels/second or something would be smoother
const ANIMATION_TIME_S: f64 = 0.5;

pub struct Warper {
    started: Instant,
    line: Option<Line>,
}

impl Warper {
    pub fn new(ctx: &EventCtx, pt: Pt2D) -> Warper {
        Warper {
            started: Instant::now(),
            line: Line::maybe_new(ctx.canvas.center_to_map_pt(), pt),
        }
    }

    pub fn event(&self, ctx: &mut EventCtx) -> Option<EventLoopMode> {
        let line = self.line.as_ref()?;

        // Weird to do stuff for any event?
        if ctx.input.nonblocking_is_update_event() {
            ctx.input.use_update_event();
        }

        let percent = elapsed_seconds(self.started) / ANIMATION_TIME_S;
        if percent >= 1.0 {
            ctx.canvas.center_on_map_pt(line.pt2());
            //ctx.primary.current_selection = Some(*id);
            None
        } else {
            ctx.canvas
                .center_on_map_pt(line.dist_along(line.length() * percent));
            Some(EventLoopMode::Animation)
        }
    }
}
