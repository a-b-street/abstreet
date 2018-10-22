use dimensioned::si;
use geom::{Angle, PolyLine, Pt2D};
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

    pub fn slice(
        &self,
        reverse: bool,
        map: &Map,
        start: si::Meter<f64>,
        end: si::Meter<f64>,
    ) -> (Trace, si::Meter<f64>) {
        if start > end || start < 0.0 * si::M || end < 0.0 * si::M {
            panic!(
                "Can't do slice [{}, {}] with reverse={} on {:?}",
                start, end, reverse, self
            );
        }

        match self {
            &Traversable::Lane(id) => if reverse {
                let pts = &map.get_l(id).lane_center_pts;
                let len = pts.length();
                let (polyline, remainder) = pts.reversed().slice(len - start, end);
                let actual_len = polyline.length();
                (
                    Trace {
                        geom: TraceGeometry::PolyLine(polyline),
                        segments: vec![TraceSegment {
                            on: *self,
                            start_dist: len - start,
                            end_dist: len - (start + actual_len),
                        }],
                    },
                    remainder,
                )
            } else {
                let (polyline, remainder) = map.get_l(id).lane_center_pts.slice(start, end);
                let actual_len = polyline.length();
                (
                    Trace {
                        geom: TraceGeometry::PolyLine(polyline),
                        segments: vec![TraceSegment {
                            on: *self,
                            start_dist: start,
                            end_dist: start + actual_len,
                        }],
                    },
                    remainder,
                )
            },
            &Traversable::Turn(id) => {
                assert!(!reverse);
                let t = map.get_t(id);
                // Don't do the epsilon comparison here... if we did, the assert_eq's in extend()
                // need to also have some buffer.
                if t.line.length() == 0.0 * si::M {
                    (
                        Trace {
                            geom: TraceGeometry::Point(t.line.pt1()),
                            segments: vec![TraceSegment {
                                on: *self,
                                start_dist: start,
                                end_dist: start,
                            }],
                        },
                        end,
                    )
                } else {
                    let (polyline, remainder) =
                        PolyLine::new(vec![t.line.pt1(), t.line.pt2()]).slice(start, end);
                    let actual_len = polyline.length();
                    (
                        Trace {
                            geom: TraceGeometry::PolyLine(polyline),
                            segments: vec![TraceSegment {
                                on: *self,
                                start_dist: start,
                                end_dist: start + actual_len,
                            }],
                        },
                        remainder,
                    )
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

pub enum TraceGeometry {
    Point(Pt2D),
    PolyLine(PolyLine),
}

pub struct Trace {
    // TODO A finalized trace is always a PolyLine... have a private builder type?
    pub geom: TraceGeometry,
    pub segments: Vec<TraceSegment>,
}

impl Trace {
    pub fn get_polyline(&self) -> &PolyLine {
        match self.geom {
            TraceGeometry::Point(_) => panic!("Trace is a point, not polyline"),
            TraceGeometry::PolyLine(ref polyline) => &polyline,
        }
    }

    pub fn endpoints(&self) -> (Pt2D, Pt2D) {
        match self.geom {
            TraceGeometry::Point(pt) => (pt, pt),
            TraceGeometry::PolyLine(ref polyline) => {
                (polyline.points()[0], *polyline.points().last().unwrap())
            }
        }
    }

    pub fn extend(mut self, other: Trace) -> Trace {
        self.geom = match (self.geom, other.geom) {
            (TraceGeometry::Point(pt1), TraceGeometry::Point(pt2)) => {
                assert_eq!(pt1, pt2);
                TraceGeometry::Point(pt1)
            }
            (TraceGeometry::Point(pt1), TraceGeometry::PolyLine(line2)) => {
                assert_eq!(pt1, line2.points()[0]);
                TraceGeometry::PolyLine(line2)
            }
            (TraceGeometry::PolyLine(line1), TraceGeometry::Point(pt2)) => {
                assert_eq!(*line1.points().last().unwrap(), pt2);
                TraceGeometry::PolyLine(line1)
            }
            (TraceGeometry::PolyLine(mut line1), TraceGeometry::PolyLine(line2)) => {
                line1.extend(line2);
                TraceGeometry::PolyLine(line1)
            }
        };
        self.segments.extend(other.segments);
        self
    }

    pub fn debug(&self) {
        println!("Trace with {} segments", self.segments.len());
        match self.geom {
            TraceGeometry::Point(pt) => {
                println!("  - Point({})", pt);
            }
            TraceGeometry::PolyLine(ref polyline) => {
                println!(
                    "  - PolyLine({} ... {})",
                    polyline.points()[0],
                    polyline.points().last().unwrap()
                );
            }
        }
        for s in &self.segments {
            println!("  - {:?} [{} to {}]", s.on, s.start_dist, s.end_dist);
        }
    }
}
