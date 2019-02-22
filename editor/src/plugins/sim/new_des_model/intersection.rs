use crate::plugins::sim::new_des_model::Queue;
use geom::Duration;
use map_model::{IntersectionID, LaneID, Map, Traversable, TurnID};
use sim::CarID;
use std::collections::{BTreeMap, HashSet};

pub struct IntersectionController {
    _id: IntersectionID,
    accepted: HashSet<(CarID, TurnID)>,
}

impl IntersectionController {
    pub fn new(id: IntersectionID) -> IntersectionController {
        IntersectionController {
            _id: id,
            accepted: HashSet::new(),
        }
    }

    // The head car calls this when they're at the end of the lane Queued.
    pub fn can_start_turn(
        &self,
        _car: CarID,
        turn: TurnID,
        queues: &BTreeMap<Traversable, Queue>,
        time: Duration,
        map: &Map,
    ) -> bool {
        // Policy: only one turn at a time, can't go until the target lane has room.
        /*if !self.accepted.is_empty() {
            return false;
        }
        // TODO This isn't strong enough -- make sure there's room for the car to immediately
        // complete the turn and get out of the intersection completely.
        if !queues[&Traversable::Lane(turn.dst)].room_at_end(time) {
            return false;
        }*/

        // Policy: allow concurrent turns that don't conflict, don't prevent target lane from
        // spilling over.
        let req_turn = map.get_t(turn);
        if self
            .accepted
            .iter()
            .any(|(_, t)| map.get_t(*t).conflicts_with(req_turn))
        {
            return false;
        }

        true
    }

    pub fn nobody_headed_towards(&self, dst_lane: LaneID) -> bool {
        !self.accepted.iter().any(|(_, turn)| turn.dst == dst_lane)
    }

    pub fn turn_started(&mut self, car: CarID, turn: TurnID) {
        self.accepted.insert((car, turn));
    }

    pub fn turn_finished(&mut self, car: CarID, turn: TurnID) {
        assert!(self.accepted.remove(&(car, turn)));
    }
}
