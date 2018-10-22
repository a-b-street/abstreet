use piston::input::Key;
use plugins::{Plugin, PluginCtx};
use sim::TripID;

#[derive(PartialEq)]
pub enum FollowState {
    Empty,
    Active(TripID),
}

impl Plugin for FollowState {
    fn event(&mut self, ctx: PluginCtx) -> bool {
        if *self == FollowState::Empty {
            if let Some(agent) = ctx.primary.current_selection.and_then(|id| id.agent_id()) {
                if let Some(trip) = ctx.primary.sim.agent_to_trip(agent) {
                    if ctx
                        .input
                        .key_pressed(Key::F, &format!("follow {:?}", agent))
                    {
                        *self = FollowState::Active(trip);
                        return true;
                    }
                }
            }
        }

        let mut quit = false;
        if let FollowState::Active(trip) = self {
            if let Some(pt) = ctx
                .primary
                .sim
                .get_canonical_point_for_trip(*trip, &ctx.primary.map)
            {
                ctx.canvas.center_on_map_pt(pt);
                quit = ctx.input.key_pressed(Key::Return, "stop following");
            } else {
                // TODO ideally they wouldnt vanish for so long according to
                // get_canonical_point_for_trip
                warn!("{} is gone... temporarily or not?", trip);
            }
        };
        if quit {
            *self = FollowState::Empty;
        }
        match self {
            FollowState::Empty => false,
            _ => true,
        }
    }
}
