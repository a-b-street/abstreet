use crate::objects::{Ctx, SIM};
use crate::plugins::{Plugin, PluginCtx};
use ezgui::{Color, GfxCtx, Text, TOP_RIGHT};
use piston::input::Key;
use sim::{ScoreSummary, Tick};

pub struct ShowScoreState {
    key: Key,
    state: State,
}

enum State {
    Inactive,
    Active(Tick, Text),
}

impl ShowScoreState {
    pub fn new(key: Key) -> ShowScoreState {
        ShowScoreState {
            key,
            state: State::Inactive,
        }
    }
}

impl Plugin for ShowScoreState {
    fn ambient_event(&mut self, ctx: &mut PluginCtx) {
        match self.state {
            State::Inactive => {
                if ctx
                    .input
                    .unimportant_key_pressed(self.key, SIM, "Show the sim info sidepanel")
                {
                    self.state = panel(ctx);
                }
            }
            State::Active(last_tick, _) => {
                if ctx
                    .input
                    .key_pressed(self.key, "Hide the sim info sidepanel")
                {
                    self.state = State::Inactive;
                } else if last_tick != ctx.primary.sim.time {
                    self.state = panel(ctx);
                }
            }
        }
    }

    fn draw(&self, g: &mut GfxCtx, ctx: &mut Ctx) {
        if let State::Active(_, ref text) = self.state {
            ctx.canvas.draw_text(g, text.clone(), TOP_RIGHT);
        }
    }
}

fn panel(ctx: &mut PluginCtx) -> State {
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
    State::Active(ctx.primary.sim.time, txt)
}

fn summarize(txt: &mut Text, summary: ScoreSummary) {
    txt.add_styled_line(
        "Walking".to_string(),
        Color::BLACK,
        Some(Color::rgba(255, 0, 0, 0.8)),
    );
    txt.add_line(format!(
        "  {}/{} trips done",
        (summary.total_walking_trips - summary.pending_walking_trips),
        summary.pending_walking_trips
    ));
    txt.add_line(format!("  {} total", summary.total_walking_trip_time));

    txt.add_styled_line(
        "Driving".to_string(),
        Color::BLACK,
        Some(Color::rgba(0, 0, 255, 0.8)),
    );
    txt.add_line(format!(
        "  {}/{} trips done",
        (summary.total_driving_trips - summary.pending_driving_trips),
        summary.pending_driving_trips
    ));
    txt.add_line(format!("  {} total", summary.total_driving_trip_time));
}
