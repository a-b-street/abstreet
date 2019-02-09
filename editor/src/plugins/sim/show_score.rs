use crate::objects::DrawCtx;
use crate::plugins::{NonblockingPlugin, PluginCtx};
use ezgui::{Color, GfxCtx, HorizontalAlignment, Text, VerticalAlignment};
use sim::{ScoreSummary, Tick};

pub struct ShowScoreState {
    last_tick: Tick,
    txt: Text,
}

impl ShowScoreState {
    pub fn new(ctx: &mut PluginCtx) -> Option<ShowScoreState> {
        if ctx.input.action_chosen("show/hide sim info sidepanel") {
            return Some(panel(ctx));
        }
        None
    }
}

impl NonblockingPlugin for ShowScoreState {
    fn nonblocking_event(&mut self, ctx: &mut PluginCtx) -> bool {
        if ctx.input.action_chosen("show/hide sim info sidepanel") {
            return false;
        }
        if self.last_tick != ctx.primary.sim.time {
            *self = panel(ctx);
        }
        true
    }

    fn draw(&self, g: &mut GfxCtx, _ctx: &DrawCtx) {
        g.draw_blocking_text(
            self.txt.clone(),
            (HorizontalAlignment::Right, VerticalAlignment::BelowTopMenu),
        );
    }
}

fn panel(ctx: &mut PluginCtx) -> ShowScoreState {
    let mut txt = Text::new();
    if let Some((s, _)) = ctx.secondary {
        // TODO More coloring
        txt.add_line(ctx.primary.sim.get_name().to_string());
        summarize(&mut txt, ctx.primary.sim.get_score());
        txt.add_line(String::new());
        txt.add_line(s.sim.get_name().to_string());
        summarize(&mut txt, s.sim.get_score());
    } else {
        summarize(&mut txt, ctx.primary.sim.get_score());
    }
    ShowScoreState {
        last_tick: ctx.primary.sim.time,
        txt,
    }
}

fn summarize(txt: &mut Text, summary: ScoreSummary) {
    txt.add_styled_line("Walking".to_string(), None, Some(Color::RED.alpha(0.8)));
    txt.add_line(format!(
        "  {}/{} trips done",
        (summary.total_walking_trips - summary.pending_walking_trips),
        summary.pending_walking_trips
    ));
    txt.add_line(format!("  {} total", summary.total_walking_trip_time));

    txt.add_styled_line("Driving".to_string(), None, Some(Color::BLUE.alpha(0.8)));
    txt.add_line(format!(
        "  {}/{} trips done",
        (summary.total_driving_trips - summary.pending_driving_trips),
        summary.pending_driving_trips
    ));
    txt.add_line(format!("  {} total", summary.total_driving_trip_time));
}
