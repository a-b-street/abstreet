use crate::{BuildingID, LaneID, LaneType, Map, TurnID};
use geom::{Angle, Distance, PolyLine, Pt2D, Speed};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
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

    pub fn pt_and_angle(&self, map: &Map) -> (Pt2D, Angle) {
        map.get_l(self.lane).dist_along(self.dist_along)
    }

    pub fn equiv_pos(&self, lane: LaneID, our_len: Distance, map: &Map) -> Position {
        let r = map.get_parent(lane);
        assert_eq!(map.get_l(self.lane).parent, r.id);

        // TODO Project perpendicular
        let len = map.get_l(lane).length();
        // The two lanes may be on opposite sides of the road; this often happens on one-ways with
        // sidewalks on both sides.
        if r.is_forwards(lane) == r.is_forwards(self.lane) {
            Position::new(lane, self.dist_along.min(len))
        } else {
            Position::new(
                lane,
                (len - self.dist_along + our_len)
                    .max(Distance::ZERO)
                    .min(len),
            )
        }
    }

    pub fn bldg_via_walking(b: BuildingID, map: &Map) -> Position {
        map.get_b(b).front_path.sidewalk
    }

    pub fn bldg_via_driving(b: BuildingID, map: &Map) -> Option<Position> {
        let bldg = map.get_b(b);
        let driving_lane = map
            .find_closest_lane(bldg.sidewalk(), vec![LaneType::Driving])
            .ok()?;
        Some(
            bldg.front_path
                .sidewalk
                .equiv_pos(driving_lane, Distance::ZERO, map),
        )
    }

    pub fn bldg_via_biking(b: BuildingID, map: &Map) -> Option<Position> {
        let bldg = map.get_b(b);
        let driving_lane = map
            .find_closest_lane(bldg.sidewalk(), vec![LaneType::Biking])
            .or_else(|_| map.find_closest_lane(bldg.sidewalk(), vec![LaneType::Driving]))
            .ok()?;
        Some(
            bldg.front_path
                .sidewalk
                .equiv_pos(driving_lane, Distance::ZERO, map),
        )
    }
}

// TODO also building paths?
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Traversable {
    Lane(LaneID),
    Turn(TurnID),
}

impl fmt::Display for Traversable {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Traversable::Lane(id) => write!(f, "Traversable::Lane({})", id.0),
            Traversable::Turn(id) => write!(
                f,
                "Traversable::Turn({}, {}, {})",
                id.src, id.dst, id.parent
            ),
        }
    }
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

    pub fn exact_slice(&self, start: Distance, end: Distance, map: &Map) -> PolyLine {
        match *self {
            Traversable::Lane(id) => map.get_l(id).lane_center_pts.exact_slice(start, end),
            Traversable::Turn(id) => map.get_t(id).geom.exact_slice(start, end),
        }
    }

    pub fn speed_limit(&self, map: &Map) -> Speed {
        match *self {
            Traversable::Lane(id) => map.get_parent(id).speed_limit,
            Traversable::Turn(id) => map.get_parent(id.dst).speed_limit,
        }
    }

    pub fn get_zorder(&self, map: &Map) -> isize {
        match *self {
            Traversable::Lane(id) => map.get_parent(id).zorder,
            Traversable::Turn(id) => map.get_i(id.parent).get_zorder(map),
        }
    }
}
