use ezgui::GfxCtx;
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
                ctx.secondary.is_some() && ctx.input.key_pressed(Key::D, "Diff all trips")
            }
            DiffAllState::Active(_) => {
                !ctx.input.key_pressed(Key::Return, "Stop diffing all trips")
            }
        };

        if active {
            let primary_sim = &ctx.primary.sim;
            let primary_map = &ctx.primary.map;
            let (secondary_sim, secondary_map) = ctx
                .secondary
                .as_ref()
                .map(|(s, _)| (&s.sim, &s.map))
                .unwrap();

            let mut same_trips = 0;
            let mut diff_trips = 0;
            *self = DiffAllState::Active(
                primary_sim
                    .get_active_trips()
                    .into_iter()
                    .filter_map(|trip| {
                        let pt1 = primary_sim.get_canonical_point_for_trip(trip, primary_map);
                        let pt2 = secondary_sim.get_canonical_point_for_trip(trip, secondary_map);
                        if pt1.is_some() && pt2.is_some() {
                            let pt1 = pt1.unwrap();
                            let pt2 = pt2.unwrap();
                            if pt1 != pt2 {
                                diff_trips += 1;
                                Some(Line::new(pt1, pt2))
                            } else {
                                same_trips += 1;
                                None
                            }
                        } else {
                            None
                        }
                    }).collect(),
            );
            ctx.osd.add_line(format!(
                "{} trips same, {} trips different",
                same_trips, diff_trips
            ));
        } else {
            *self = DiffAllState::Inactive;
        }

        active
    }

    fn draw(&self, g: &mut GfxCtx, _ctx: Ctx) {
        if let DiffAllState::Active(ref lines) = self {
            for line in lines {
                // TODO move constants
                g.draw_line([1.0, 1.0, 0.0, 1.0], LANE_THICKNESS, line);
            }
        }
    }
}
