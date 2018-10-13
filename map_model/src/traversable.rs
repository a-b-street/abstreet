use dimensioned::si;
use geom::{Angle, PolyLine, Pt2D, EPSILON_DIST};
use {LaneID, Map, TurnID};

// TODO this probably doesn't belong in map model after all.

// TODO also building paths?
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
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

    // Returns None if the traversable is actually 0 length, as some turns are.
    pub fn slice(
        &self,
        reverse: bool,
        map: &Map,
        start: si::Meter<f64>,
        end: si::Meter<f64>,
    ) -> Option<(Trace, si::Meter<f64>)> {
        match self {
            &Traversable::Lane(id) => if reverse {
                let pts = &map.get_l(id).lane_center_pts;
                let len = pts.length();
                let (polyline, remainder) = pts.reversed().slice(start, end);
                Some((
                    Trace {
                        polyline,
                        segments: vec![TraceSegment {
                            on: *self,
                            start_dist: len - start,
                            end_dist: len - end,
                        }],
                    },
                    remainder,
                ))
            } else {
                let (polyline, remainder) = map.get_l(id).lane_center_pts.slice(start, end);
                Some((
                    Trace {
                        polyline,
                        segments: vec![TraceSegment {
                            on: *self,
                            start_dist: start,
                            end_dist: end,
                        }],
                    },
                    remainder,
                ))
            },
            &Traversable::Turn(id) => {
                assert!(!reverse);
                let t = map.get_t(id);
                if t.line.length() <= EPSILON_DIST {
                    None
                } else {
                    let (polyline, remainder) =
                        PolyLine::new(vec![t.line.pt1(), t.line.pt2()]).slice(start, end);
                    Some((
                        Trace {
                            polyline,
                            segments: vec![TraceSegment {
                                on: *self,
                                start_dist: start,
                                end_dist: end,
                            }],
                        },
                        remainder,
                    ))
                }
            }
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

pub struct TraceSegment {
    pub on: Traversable,
    pub start_dist: si::Meter<f64>,
    pub end_dist: si::Meter<f64>,
}

pub struct Trace {
    pub polyline: PolyLine,
    pub segments: Vec<TraceSegment>,
}

impl Trace {
    pub fn extend(&mut self, other: Trace) {
        self.polyline.extend(other.polyline);
        self.segments.extend(other.segments);
    }
}
