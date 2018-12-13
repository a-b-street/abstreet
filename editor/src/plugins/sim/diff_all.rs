use crate::objects::Ctx;
use crate::plugins::{Plugin, PluginCtx};
use ezgui::GfxCtx;
use geom::Line;
use map_model::LANE_THICKNESS;
use piston::input::Key;
use sim::{Sim, Tick};

pub struct DiffAllState {
    time: Tick,
    same_trips: usize,
    // TODO Or do we want to augment DrawCars and DrawPeds, so we get automatic quadtree support?
    lines: Vec<Line>,
}

impl DiffAllState {
    pub fn new(key: Key, ctx: &mut PluginCtx) -> Option<DiffAllState> {
        if ctx.primary.current_selection.is_none() && ctx.input.key_pressed(key, "Diff all trips") {
            return Some(diff_all(
                &ctx.primary.sim,
                ctx.secondary.as_ref().map(|(s, _)| &s.sim).unwrap(),
            ));
        }
        None
    }
}

impl Plugin for DiffAllState {
    fn blocking_event(&mut self, ctx: &mut PluginCtx) -> bool {
        if ctx.input.key_pressed(Key::Return, "Stop diffing all trips") {
            return false;
        }
        if self.time != ctx.primary.sim.time {
            *self = diff_all(
                &ctx.primary.sim,
                ctx.secondary.as_ref().map(|(s, _)| &s.sim).unwrap(),
            );
        }
        ctx.hints.osd.add_line(format!(
            "{} trips same, {} trips different",
            self.same_trips,
            self.lines.len()
        ));
        true
    }

    fn draw(&self, g: &mut GfxCtx, ctx: &Ctx) {
        for line in &self.lines {
            g.draw_line(ctx.cs.get("diff agents line"), LANE_THICKNESS, line);
        }
    }
}

fn diff_all(primary_sim: &Sim, secondary_sim: &Sim) -> DiffAllState {
    let stats1 = primary_sim.get_stats();
    let stats2 = secondary_sim.get_stats();
    let mut same_trips = 0;
    let mut lines: Vec<Line> = Vec::new();
    for (trip, pt1) in &stats1.canonical_pt_per_trip {
        if let Some(pt2) = stats2.canonical_pt_per_trip.get(trip) {
            if pt1 == pt2 {
                same_trips += 1;
            } else {
                lines.push(Line::new(*pt1, *pt2));
            }
        }
    }
    DiffAllState {
        time: primary_sim.time,
        same_trips,
        lines,
    }
}
