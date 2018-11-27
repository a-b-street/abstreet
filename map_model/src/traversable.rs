use dimensioned::si;
use geom::{Angle, Pt2D};
use std::fmt;
use {LaneID, Map, TurnID};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Position {
    // Don't let callers construct a Position directly, so it's easy to find callers of new().
    lane: LaneID,
    dist_along: si::Meter<f64>,
}

impl fmt::Display for Position {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Position({}, {})", self.lane, self.dist_along)
    }
}

impl Position {
    pub fn new(lane: LaneID, dist_along: si::Meter<f64>) -> Position {
        Position { lane, dist_along }
    }

    pub fn lane(&self) -> LaneID {
        self.lane
    }

    pub fn dist_along(&self) -> si::Meter<f64> {
        self.dist_along
    }

    pub fn pt_and_angle(&self, map: &Map) -> (Pt2D, Angle) {
        map.get_l(self.lane).dist_along(self.dist_along)
    }

    pub fn equiv_pos(&self, lane: LaneID, map: &Map) -> Position {
        // TODO Assert lane is in the same road / side of the road
        let len = map.get_l(lane).length();
        // TODO Project perpendicular
        if self.dist_along < len {
            Position::new(lane, self.dist_along)
        } else {
            Position::new(lane, len)
        }
    }
}

// TODO also building paths?
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Traversable {
    Lane(LaneID),
    Turn(TurnID),
}

impl Traversable {
    pub fn as_lane(&self) -> LaneID {
        match self {
            &Traversable::Lane(id) => id,
            &Traversable::Turn(_) => panic!("not a lane"),
        }
    }

    pub fn as_turn(&self) -> TurnID {
        match self {
            &Traversable::Turn(id) => id,
            &Traversable::Lane(_) => panic!("not a turn"),
        }
    }

    pub fn maybe_turn(&self) -> Option<TurnID> {
        match self {
            &Traversable::Turn(id) => Some(id),
            &Traversable::Lane(_) => None,
        }
    }

    pub fn maybe_lane(&self) -> Option<LaneID> {
        match self {
            &Traversable::Turn(_) => None,
            &Traversable::Lane(id) => Some(id),
        }
    }

    // TODO Just expose the PolyLine instead of all these layers of helpers
    pub fn length(&self, map: &Map) -> si::Meter<f64> {
        match self {
            &Traversable::Lane(id) => map.get_l(id).length(),
            &Traversable::Turn(id) => map.get_t(id).length(),
        }
    }

    pub fn dist_along(&self, dist: si::Meter<f64>, map: &Map) -> (Pt2D, Angle) {
        match self {
            &Traversable::Lane(id) => map.get_l(id).dist_along(dist),
            &Traversable::Turn(id) => map.get_t(id).dist_along(dist),
        }
    }

    pub fn speed_limit(&self, map: &Map) -> si::MeterPerSecond<f64> {
        match self {
            &Traversable::Lane(id) => map.get_parent(id).get_speed_limit(),
            &Traversable::Turn(id) => map.get_parent(id.dst).get_speed_limit(),
        }
    }
}
