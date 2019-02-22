use crate::{LaneID, Map, TurnID};
use geom::{Angle, Distance, PolyLine, Pt2D, Speed};
use serde_derive::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Position {
    // Don't let callers construct a Position directly, so it's easy to find callers of new().
    lane: LaneID,
    dist_along: Distance,
}

impl fmt::Display for Position {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Position({}, {})", self.lane, self.dist_along)
    }
}

impl Position {
    pub fn new(lane: LaneID, dist_along: Distance) -> Position {
        Position { lane, dist_along }
    }

    pub fn lane(&self) -> LaneID {
        self.lane
    }

    pub fn dist_along(&self) -> Distance {
        self.dist_along
    }

    pub fn pt(&self, map: &Map) -> Pt2D {
        map.get_l(self.lane).dist_along(self.dist_along).0
    }

    pub fn equiv_pos(&self, lane: LaneID, map: &Map) -> Position {
        let r = map.get_parent(lane);
        assert_eq!(map.get_l(self.lane).parent, r.id);
        assert_eq!(r.is_forwards(lane), r.is_forwards(self.lane));

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
        match *self {
            Traversable::Lane(id) => id,
            Traversable::Turn(_) => panic!("not a lane"),
        }
    }

    pub fn as_turn(&self) -> TurnID {
        match *self {
            Traversable::Turn(id) => id,
            Traversable::Lane(_) => panic!("not a turn"),
        }
    }

    pub fn maybe_turn(&self) -> Option<TurnID> {
        match *self {
            Traversable::Turn(id) => Some(id),
            Traversable::Lane(_) => None,
        }
    }

    pub fn maybe_lane(&self) -> Option<LaneID> {
        match *self {
            Traversable::Turn(_) => None,
            Traversable::Lane(id) => Some(id),
        }
    }

    // TODO Just expose the PolyLine instead of all these layers of helpers
    pub fn length(&self, map: &Map) -> Distance {
        match *self {
            Traversable::Lane(id) => map.get_l(id).length(),
            Traversable::Turn(id) => map.get_t(id).geom.length(),
        }
    }

    pub fn dist_along(&self, dist: Distance, map: &Map) -> (Pt2D, Angle) {
        match *self {
            Traversable::Lane(id) => map.get_l(id).dist_along(dist),
            Traversable::Turn(id) => map.get_t(id).geom.dist_along(dist),
        }
    }

    pub fn slice(&self, start: Distance, end: Distance, map: &Map) -> Option<(PolyLine, Distance)> {
        match *self {
            Traversable::Lane(id) => map.get_l(id).lane_center_pts.slice(start, end),
            Traversable::Turn(id) => map.get_t(id).geom.slice(start, end),
        }
    }

    pub fn speed_limit(&self, map: &Map) -> Speed {
        match *self {
            Traversable::Lane(id) => map.get_parent(id).get_speed_limit(),
            Traversable::Turn(id) => map.get_parent(id.dst).get_speed_limit(),
        }
    }

    pub fn get_zorder(&self, map: &Map) -> isize {
        match *self {
            Traversable::Lane(id) => map.get_parent(id).get_zorder(),
            Traversable::Turn(id) => map.get_i(id.parent).get_zorder(map),
        }
    }
}
