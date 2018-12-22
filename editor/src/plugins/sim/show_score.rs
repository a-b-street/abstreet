use crate::objects::Ctx;
use crate::plugins::{Plugin, PluginCtx};
use ezgui::{Color, GfxCtx, Text, TOP_RIGHT};
use sim::{ScoreSummary, Tick};

pub enum ShowScoreState {
    Inactive,
    Active(Tick, Text),
}

impl ShowScoreState {
    pub fn new() -> ShowScoreState {
        ShowScoreState::Inactive
    }
}

impl Plugin for ShowScoreState {
    fn ambient_event(&mut self, ctx: &mut PluginCtx) {
        match self {
            ShowScoreState::Inactive => {
                if ctx.input.action_chosen("show/hide sim info sidepanel") {
                    *self = panel(ctx);
                }
            }
            ShowScoreState::Active(last_tick, _) => {
                if ctx.input.action_chosen("show/hide sim info sidepanel") {
                    *self = ShowScoreState::Inactive;
                } else if *last_tick != ctx.primary.sim.time {
                    *self = panel(ctx);
                }
            }
        }
    }

    fn draw(&self, g: &mut GfxCtx, ctx: &Ctx) {
        if let ShowScoreState::Active(_, ref text) = self {
            ctx.canvas.draw_text(g, text.clone(), TOP_RIGHT);
        }
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
    ShowScoreState::Active(ctx.primary.sim.time, txt)
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
