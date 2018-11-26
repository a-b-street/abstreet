use dimensioned::si;
use geom::{Angle, Pt2D};
use {LaneID, Map, TurnID};

// TODO this probably doesn't belong in map model after all.

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
