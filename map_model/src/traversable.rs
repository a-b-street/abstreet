use std::fmt;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use geom::{Angle, Distance, PolyLine, Pt2D, Speed};

use crate::{Direction, LaneID, Map, PathConstraints, TurnID};

/// Represents a specific point some distance along a lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
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

    pub fn start(lane: LaneID) -> Position {
        Position {
            lane,
            dist_along: Distance::ZERO,
        }
    }

    pub fn end(lane: LaneID, map: &Map) -> Position {
        Position {
            lane,
            dist_along: map.get_l(lane).length(),
        }
    }

    pub fn lane(&self) -> LaneID {
        self.lane
    }

    pub fn dist_along(&self) -> Distance {
        self.dist_along
    }

    pub fn pt(&self, map: &Map) -> Pt2D {
        match map
            .get_l(self.lane)
            .lane_center_pts
            .dist_along(self.dist_along)
        {
            Ok((pt, _)) => pt,
            Err(err) => panic!("{} invalid: {}", self, err),
        }
    }

    pub fn pt_and_angle(&self, map: &Map) -> (Pt2D, Angle) {
        match map
            .get_l(self.lane)
            .lane_center_pts
            .dist_along(self.dist_along)
        {
            Ok(pair) => pair,
            Err(err) => panic!("{} invalid: {}", self, err),
        }
    }

    pub fn equiv_pos(&self, lane: LaneID, map: &Map) -> Position {
        self.equiv_pos_for_long_object(lane, Distance::ZERO, map)
    }
    pub fn equiv_pos_for_long_object(
        &self,
        lane: LaneID,
        our_len: Distance,
        map: &Map,
    ) -> Position {
        let r = map.get_parent(lane);
        assert_eq!(map.get_l(self.lane).parent, r.id);

        // TODO Project perpendicular
        let len = map.get_l(lane).length();
        // The two lanes may be on opposite sides of the road; this often happens on one-ways with
        // sidewalks on both sides.
        if r.dir(lane) == r.dir(self.lane) {
            Position::new(lane, self.dist_along.min(len))
        } else {
            Position::new(
                lane,
                // TODO I don't understand what this is doing anymore in the one case, revisit
                (len - self.dist_along + our_len)
                    .max(Distance::ZERO)
                    .min(len),
            )
        }
    }
    pub fn min_dist(mut self, dist_along: Distance, map: &Map) -> Option<Position> {
        if self.dist_along >= dist_along {
            return Some(self);
        }
        if map.get_l(self.lane).length() < dist_along {
            return None;
        }
        self.dist_along = dist_along;
        Some(self)
    }
    pub fn buffer_dist(mut self, buffer: Distance, map: &Map) -> Option<Position> {
        let len = map.get_l(self.lane).length();
        if len <= buffer * 2.0 {
            return None;
        }
        self.dist_along = self.dist_along.max(buffer).min(len - buffer);
        Some(self)
    }
}

/// Either a lane or a turn, where most movement happens.
// TODO Consider adding building and parking lot driveways here.
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

    pub fn dist_along(&self, dist: Distance, map: &Map) -> Result<(Pt2D, Angle)> {
        match *self {
            Traversable::Lane(id) => map.get_l(id).lane_center_pts.dist_along(dist),
            Traversable::Turn(id) => map.get_t(id).geom.dist_along(dist),
        }
    }

    pub fn slice(&self, start: Distance, end: Distance, map: &Map) -> Result<(PolyLine, Distance)> {
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

    pub fn get_zorder(&self, map: &Map) -> isize {
        match *self {
            Traversable::Lane(id) => map.get_parent(id).zorder,
            Traversable::Turn(id) => map.get_i(id.parent).get_zorder(map),
        }
    }

    /// The single definitive place to determine how fast somebody could go along a single road or
    /// turn. This should be used for pathfinding and simulation.
    pub fn max_speed_along(
        &self,
        max_speed_on_flat_ground: Option<Speed>,
        constraints: PathConstraints,
        map: &Map,
    ) -> Speed {
        let base = match self {
            Traversable::Lane(l) => {
                let road = map.get_parent(*l);
                let percent_incline = if road.dir(*l) == Direction::Fwd {
                    road.percent_incline
                } else {
                    -road.percent_incline
                };

                if constraints == PathConstraints::Bike {
                    // We assume every bike has a max_speed defined.
                    bike_speed_on_incline(max_speed_on_flat_ground.unwrap(), percent_incline)
                } else if constraints == PathConstraints::Pedestrian {
                    // We assume every pedestrian has a max_speed defined.
                    walking_speed_on_incline(max_speed_on_flat_ground.unwrap(), percent_incline)
                } else {
                    // Incline doesn't affect cars, buses, or trains
                    road.speed_limit
                }
            }
            // TODO Ignore elevation on turns?
            Traversable::Turn(t) => map
                .get_parent(t.src)
                .speed_limit
                .min(map.get_parent(t.dst).speed_limit),
        };
        if let Some(s) = max_speed_on_flat_ground {
            base.min(s)
        } else {
            base
        }
    }
}

// 10mph
pub const MAX_BIKE_SPEED: Speed = Speed::const_meters_per_second(4.4704);
// 3mph
pub const MAX_WALKING_SPEED: Speed = Speed::const_meters_per_second(1.34);

fn bike_speed_on_incline(max_speed: Speed, percent_incline: f64) -> Speed {
    // There doesn't seem to be a straightforward way of calculating how an "average" cyclist's
    // speed is affected by hills. http://www.kreuzotter.de/english/espeed.htm has lots of detail,
    // but we'd need to guess values like body size, type of bike, etc.
    // https://github.com/ibi-group/OpenTripPlanner/blob/65dcf0a4142e31028cf9d1b2c15ad32dd1084252/src/main/java/org/opentripplanner/routing/edgetype/StreetEdge.java#L934-L1082
    // is built from this, but seems to be more appropriate for motorized micromobility devices
    // like e-scooters.

    // So, we'll adapt the table from Valhalla --
    // https://valhalla.readthedocs.io/en/latest/sif/elevation_costing/ describes how this works.
    // Their "weighted grade" should be roughly equivalent to how the elevation_lookups package we
    // use calculates things.  This table comes from
    // https://github.com/valhalla/valhalla/blob/f899a940ccbd0bc986769197dec5bb9383014afb/src/sif/bicyclecost.cc#L139.
    // Valhalla is MIT licensed: https://github.com/valhalla/valhalla/blob/master/COPYING.

    // TODO Could binary search or do something a bit faster here, but doesn't matter much
    let pct = percent_incline * 100.0;
    for (grade, factor) in vec![
        (-10.0, 2.2),
        (-8.0, 2.0),
        (-6.5, 1.9),
        (-5.0, 1.7),
        (-3.0, 1.4),
        (-1.5, 1.2),
        (0.0, 1.0),
        (1.5, 0.95),
        (3.0, 0.85),
        (5.0, 0.75),
        (6.5, 0.65),
        (8.0, 0.55),
        (10.0, 0.5),
        (11.5, 0.45),
        (13.0, 0.4),
    ] {
        if pct <= grade {
            return factor * max_speed;
        }
    }
    // The last pair is a factor of 0.3 for grades of 15%, but we'll use it for anything steeper
    // than 15%
    0.3 * max_speed
}

fn walking_speed_on_incline(max_speed: Speed, _percent_incline: f64) -> Speed {
    // TODO Incorporate percent_incline here
    max_speed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bike_speed_on_incline() {
        let base_speed = Speed::miles_per_hour(3.0);
        assert_approx_eq(
            Speed::miles_per_hour(3.0),
            bike_speed_on_incline(base_speed, 0.0),
        );
        assert_approx_eq(
            Speed::miles_per_hour(6.6),
            bike_speed_on_incline(base_speed, -0.15),
        );
        assert_approx_eq(
            Speed::miles_per_hour(0.9),
            bike_speed_on_incline(base_speed, 0.15),
        );
    }

    fn assert_approx_eq(s1: Speed, s2: Speed) {
        if (s1.inner_meters_per_second() - s2.inner_meters_per_second()).abs() > 0.001 {
            panic!("{:?} != {:?}", s1, s2);
        }
    }
}
