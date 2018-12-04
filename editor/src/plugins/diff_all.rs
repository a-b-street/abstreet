use ezgui::{Color, GfxCtx};
use geom::Line;
use map_model::LANE_THICKNESS;
use objects::Ctx;
use piston::input::Key;
use plugins::{Plugin, PluginCtx};
use sim::{Sim, Tick};

pub enum DiffAllState {
    Inactive,
    // TODO Or do we want to augment DrawCars and DrawPeds, so we get automatic quadtree support?
    Active {
        time: Tick,
        same_trips: usize,
        lines: Vec<Line>,
    },
}

impl DiffAllState {
    pub fn new() -> DiffAllState {
        DiffAllState::Inactive
    }
}

impl Plugin for DiffAllState {
    fn event(&mut self, ctx: PluginCtx) -> bool {
        let primary_sim = &ctx.primary.sim;

        let mut new_state: Option<DiffAllState> = None;
        match self {
            DiffAllState::Inactive => {
                if ctx.secondary.is_some()
                    && ctx.primary.current_selection.is_none()
                    && ctx.input.key_pressed(Key::D, "Diff all trips")
                {
                    let secondary_sim = ctx.secondary.as_ref().map(|(s, _)| &s.sim).unwrap();
                    new_state = Some(diff_all(primary_sim, secondary_sim));
                }
            }
            DiffAllState::Active { time, .. } => {
                if ctx.input.key_pressed(Key::Return, "Stop diffing all trips") {
                    new_state = Some(DiffAllState::Inactive);
                }
                if *time != ctx.primary.sim.time {
                    let secondary_sim = ctx.secondary.as_ref().map(|(s, _)| &s.sim).unwrap();
                    new_state = Some(diff_all(primary_sim, secondary_sim));
                }
            }
        };
        if let Some(s) = new_state {
            *self = s;
        }

        if let DiffAllState::Active {
            same_trips,
            ref lines,
            ..
        } = self
        {
            ctx.hints.osd.add_line(format!(
                "{} trips same, {} trips different",
                same_trips,
                lines.len()
            ));
            true
        } else {
            false
        }
    }

    fn draw(&self, g: &mut GfxCtx, ctx: Ctx) {
        if let DiffAllState::Active { ref lines, .. } = self {
            for line in lines {
                g.draw_line(
                    ctx.cs.get("diff agents line", Color::YELLOW),
                    LANE_THICKNESS,
                    line,
                );
            }
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
    DiffAllState::Active {
        time: primary_sim.time,
        same_trips,
        lines,
    }
}
