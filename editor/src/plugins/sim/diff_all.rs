use crate::objects::DrawCtx;
use crate::plugins::{NonblockingPlugin, PluginCtx};
use ezgui::GfxCtx;
use geom::Line;
use map_model::LANE_THICKNESS;
use sim::Tick;

pub struct DiffAllState {
    time: Tick,
    same_trips: usize,
    // TODO Or do we want to augment DrawCars and DrawPeds, so we get automatic quadtree support?
    lines: Vec<Line>,
}

impl DiffAllState {
    pub fn new(ctx: &mut PluginCtx) -> Option<DiffAllState> {
        if ctx.primary.current_selection.is_none() && ctx.input.action_chosen("diff all A/B trips")
        {
            return Some(diff_all(ctx));
        }
        None
    }
}

impl NonblockingPlugin for DiffAllState {
    fn nonblocking_event(&mut self, ctx: &mut PluginCtx) -> bool {
        if self.time != ctx.primary.sim.time {
            *self = diff_all(ctx);
        }

        ctx.input.set_mode_with_prompt(
            "A/B All Trips Explorer",
            format!(
                "Comparing all A/B trips: {} same, {} difference",
                self.same_trips,
                self.lines.len()
            ),
            &ctx.canvas,
        );
        if ctx.input.modal_action("quit") {
            return false;
        }
        true
    }

    fn draw(&self, g: &mut GfxCtx, ctx: &DrawCtx) {
        for line in &self.lines {
            g.draw_line(ctx.cs.get("diff agents line"), LANE_THICKNESS, line);
        }
    }
}

fn diff_all(ctx: &mut PluginCtx) -> DiffAllState {
    let stats1 = ctx.primary.sim.get_stats(&ctx.primary.map);
    let stats2 = ctx
        .secondary
        .as_mut()
        .map(|(s, _)| s.sim.get_stats(&s.map))
        .unwrap();
    let mut same_trips = 0;
    let mut lines: Vec<Line> = Vec::new();
    for (trip, pt1) in &stats1.canonical_pt_per_trip {
        if let Some(pt2) = stats2.canonical_pt_per_trip.get(trip) {
            if let Some(l) = Line::maybe_new(*pt1, *pt2) {
                lines.push(l);
            } else {
                same_trips += 1;
            }
        }
    }
    DiffAllState {
        time: ctx.primary.sim.time,
        same_trips,
        lines,
    }
}
