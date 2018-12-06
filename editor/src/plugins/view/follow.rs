use piston::input::Key;
use plugins::{Plugin, PluginCtx};
use sim::TripID;

pub struct FollowState {
    trip: Option<TripID>,
}

impl FollowState {
    pub fn new() -> FollowState {
        FollowState { trip: None }
    }
}

impl Plugin for FollowState {
    fn ambient_event(&mut self, ctx: &mut PluginCtx) {
        if self.trip.is_none() {
            if let Some(agent) = ctx.primary.current_selection.and_then(|id| id.agent_id()) {
                if let Some(trip) = ctx.primary.sim.agent_to_trip(agent) {
                    if ctx.input.key_pressed(Key::F, &format!("follow {}", agent)) {
                        self.trip = Some(trip);
                    }
                }
            }
        }

        if let Some(trip) = self.trip {
            if let Some(pt) = ctx.primary.sim.get_stats().canonical_pt_per_trip.get(&trip) {
                ctx.canvas.center_on_map_pt(*pt);
            } else {
                // TODO ideally they wouldnt vanish for so long according to
                // get_canonical_point_for_trip
                warn!("{} is gone... temporarily or not?", trip);
            }
            if ctx.input.key_pressed(Key::Return, "stop following") {
                self.trip = None;
            }
        }
    }
}
