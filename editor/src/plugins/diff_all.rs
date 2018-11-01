use ezgui::{Color, GfxCtx};
use geom::Line;
use map_model::LANE_THICKNESS;
use objects::Ctx;
use piston::input::Key;
use plugins::{Plugin, PluginCtx};

pub enum DiffAllState {
    Inactive,
    // TODO Or do we want to augment DrawCars and DrawPeds, so we get automatic quadtree support?
    Active(Vec<Line>),
}

impl DiffAllState {
    pub fn new() -> DiffAllState {
        DiffAllState::Inactive
    }
}

impl Plugin for DiffAllState {
    fn event(&mut self, ctx: PluginCtx) -> bool {
        let active = match self {
            DiffAllState::Inactive => {
                ctx.secondary.is_some()
                    && ctx.primary.current_selection.is_none()
                    && ctx.input.key_pressed(Key::D, "Diff all trips")
            }
            DiffAllState::Active(_) => {
                !ctx.input.key_pressed(Key::Return, "Stop diffing all trips")
            }
        };

        if active {
            let primary_sim = &ctx.primary.sim;
            let secondary_sim = ctx.secondary.as_ref().map(|(s, _)| &s.sim).unwrap();

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
            ctx.osd.add_line(format!(
                "{} trips same, {} trips different",
                same_trips,
                lines.len()
            ));
            *self = DiffAllState::Active(lines);
        } else {
            *self = DiffAllState::Inactive;
        }

        active
    }

    fn draw(&self, g: &mut GfxCtx, ctx: Ctx) {
        if let DiffAllState::Active(ref lines) = self {
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
