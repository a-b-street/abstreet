use crate::plugins::sim::new_des_model::Queue;
use geom::Duration;
use map_model::{IntersectionID, LaneID, Traversable, TurnID};
use sim::CarID;
use std::collections::BTreeMap;

pub struct IntersectionController {
    pub id: IntersectionID,
    pub accepted: Option<(CarID, TurnID)>,
}

impl IntersectionController {
    // The head car calls this when they're at the end of the lane Queued.
    pub fn can_start_turn(
        &self,
        _car: CarID,
        turn: TurnID,
        queues: &BTreeMap<Traversable, Queue>,
        time: Duration,
    ) -> bool {
        if self.accepted.is_some() {
            return false;
        }
        if !queues[&Traversable::Lane(turn.dst)].room_at_end(time) {
            return false;
        }
        true
    }

    pub fn nobody_headed_towards(&self, dst_lane: LaneID) -> bool {
        if let Some((_, turn)) = self.accepted {
            turn.dst != dst_lane
        } else {
            true
        }
    }
}
