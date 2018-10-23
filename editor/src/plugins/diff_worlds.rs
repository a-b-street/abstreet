use ezgui::GfxCtx;
use geom::Line;
use map_model::LANE_THICKNESS;
use objects::Ctx;
use piston::input::Key;
use plugins::{Plugin, PluginCtx};
use sim::TripID;

pub enum DiffWorldsState {
    Inactive,
    // The Line just points from the agent in the primary sim to the agent in the secondary. It
    // might be temporarily not present during mode changes.
    Active(TripID, Option<Line>),
}

impl DiffWorldsState {
    pub fn new() -> DiffWorldsState {
        DiffWorldsState::Inactive
    }
}

impl Plugin for DiffWorldsState {
    fn event(&mut self, ctx: PluginCtx) -> bool {
        let (input, primary, secondary) = (ctx.input, &ctx.primary, ctx.secondary);

        let mut maybe_trip: Option<TripID> = None;
        match self {
            DiffWorldsState::Inactive => {
                if secondary.is_some() {
                    if let Some(id) = primary.current_selection.and_then(|id| id.agent_id()) {
                        if let Some(trip) = primary.sim.agent_to_trip(id) {
                            if input.key_pressed(Key::B, &format!("Show {}'s parallel world", trip))
                            {
                                maybe_trip = Some(trip);
                            }
                        }
                    }
                }
            }
            DiffWorldsState::Active(trip, _) => {
                if input.key_pressed(
                    Key::Return,
                    &format!("Stop showing {}'s parallel world", trip),
                ) {
                    maybe_trip = None;
                } else {
                    maybe_trip = Some(*trip);
                }
            }
        }

        if let Some(id) = maybe_trip {
            let pt1 = primary.sim.get_canonical_point_for_trip(id, &primary.map);
            let pt2 = secondary
                .as_ref()
                .and_then(|(s, _)| s.sim.get_canonical_point_for_trip(id, &s.map));
            if pt1.is_some() && pt2.is_some() {
                *self = DiffWorldsState::Active(id, Some(Line::new(pt1.unwrap(), pt2.unwrap())));
            } else {
                warn!(
                    "{} isn't present in both sims",
                    id
                );
                *self = DiffWorldsState::Active(id, None);
            }
        } else {
            *self = DiffWorldsState::Inactive;
        }

        match self {
            DiffWorldsState::Inactive => false,
            _ => true,
        }
    }

    fn draw(&self, g: &mut GfxCtx, _ctx: Ctx) {
        if let DiffWorldsState::Active(_, Some(ref line)) = self {
            // TODO move constants
            g.draw_line([1.0, 1.0, 0.0, 1.0], LANE_THICKNESS, line);
        }
    }
}
