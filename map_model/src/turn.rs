use crate::{IntersectionID, LaneID, Map, RoadID, LANE_THICKNESS};
use abstutil::MultiMap;
use geom::{Angle, Distance, PolyLine, Pt2D};
use serde_derive::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

// Turns are uniquely identified by their (src, dst) lanes and their parent intersection.
// Intersection is needed to distinguish crosswalks that exist at two ends of a sidewalk.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TurnID {
    pub parent: IntersectionID,
    // src and dst must both belong to parent. No guarantees that src is incoming and dst is
    // outgoing for turns between sidewalks.
    pub src: LaneID,
    pub dst: LaneID,
}

impl fmt::Display for TurnID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "TurnID({}, {}, {})", self.src, self.dst, self.parent)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialOrd, Ord, PartialEq, Serialize, Deserialize)]
pub enum TurnType {
    Crosswalk,
    SharedSidewalkCorner,
    // These are for vehicle turns
    Straight,
    LaneChangeLeft,
    LaneChangeRight,
    Right,
    Left,
}

impl TurnType {
    pub fn from_angles(from: Angle, to: Angle) -> TurnType {
        let diff = from.shortest_rotation_towards(to).normalized_degrees();
        if diff < 10.0 || diff > 350.0 {
            TurnType::Straight
        } else if diff > 180.0 {
            // Clockwise rotation
            TurnType::Right
        } else {
            // Counter-clockwise rotation
            TurnType::Left
        }
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, Copy, PartialOrd)]
pub enum TurnPriority {
    // For stop signs: Can't currently specify this!
    // For traffic signals: Can't do this turn right now.
    Banned,
    // For stop signs: cars have to stop before doing this turn, and are accepted with the lowest priority.
    // For traffic signals: Cars can do this immediately if there are no previously accepted conflicting turns.
    Yield,
    // For stop signs: cars can do this without stopping. These can conflict!
    // For traffic signals: Must be non-conflicting.
    Protected,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Turn {
    pub id: TurnID,
    pub turn_type: TurnType,
    // TODO Some turns might not actually have geometry. Currently encoded by two equal points.
    // Represent more directly?
    pub geom: PolyLine,
    // Empty except for TurnType::Crosswalk.
    pub other_crosswalk_ids: BTreeSet<TurnID>,

    // Just for convenient debugging lookup.
    pub lookup_idx: usize,
}

impl Turn {
    pub fn conflicts_with(&self, other: &Turn) -> bool {
        if self.turn_type == TurnType::SharedSidewalkCorner
            || other.turn_type == TurnType::SharedSidewalkCorner
        {
            return false;
        }
        if self.id == other.id {
            return false;
        }
        if self.between_sidewalks() && other.between_sidewalks() {
            return false;
        }

        if self.geom.first_pt() == other.geom.first_pt() {
            return false;
        }
        if self.geom.last_pt() == other.geom.last_pt() {
            return true;
        }
        self.geom.intersection(&other.geom).is_some()
    }

    // TODO What should this be for zero-length turns? Probably src's pt1 to dst's pt2 or
    // something.
    pub fn angle(&self) -> Angle {
        self.geom.first_pt().angle_to(self.geom.last_pt())
    }

    pub fn between_sidewalks(&self) -> bool {
        self.turn_type == TurnType::SharedSidewalkCorner || self.turn_type == TurnType::Crosswalk
    }

    pub fn dump_debug(&self) {
        println!("{}", abstutil::to_json(self));
    }
}

// TODO Unclear how this plays with different lane types
#[derive(Clone, PartialEq)]
pub struct TurnGroup {
    pub from: RoadID,
    pub to: RoadID,
    // If this is true, there's only one member. Separate TurnGroups for each side of a crosswalk!
    pub is_crosswalk: bool,

    // Derived
    pub members: Vec<TurnID>,
}

impl TurnGroup {
    pub fn for_i(i: IntersectionID, map: &Map) -> Vec<TurnGroup> {
        let mut results = Vec::new();
        let mut groups: MultiMap<(RoadID, RoadID), TurnID> = MultiMap::new();
        for turn in map.get_turns_in_intersection(i) {
            let from = map.get_l(turn.id.src).parent;
            let to = map.get_l(turn.id.dst).parent;
            match turn.turn_type {
                TurnType::SharedSidewalkCorner => {}
                TurnType::Crosswalk => {
                    results.push(TurnGroup {
                        from,
                        to,
                        is_crosswalk: true,
                        members: vec![turn.id],
                    });
                }
                _ => {
                    groups.insert((from, to), turn.id);
                }
            }
        }
        for ((from, to), members) in groups.consume() {
            results.push(TurnGroup {
                from,
                to,
                is_crosswalk: false,
                members: members.into_iter().collect(),
            });
        }
        results
    }

    pub fn geom(&self, map: &Map) -> PolyLine {
        let polylines: Vec<&PolyLine> = self.members.iter().map(|t| &map.get_t(*t).geom).collect();
        if self.is_crosswalk {
            return polylines[0].clone();
        }

        let num_pts = polylines[0].points().len();
        for pl in &polylines {
            if num_pts != pl.points().len() {
                println!(
                    "TurnGroup between {} and {} can't make nice geometry",
                    self.from, self.to
                );
                return polylines[0].clone();
            }
        }

        let mut pts = Vec::new();
        for idx in 0..num_pts {
            pts.push(Pt2D::center(
                &polylines.iter().map(|pl| pl.points()[idx]).collect(),
            ));
        }
        PolyLine::new(pts)
    }

    pub fn angle(&self, map: &Map) -> Angle {
        map.get_t(self.members[0]).angle()
    }

    // Polyline points FROM intersection
    pub fn src_center_and_width(&self, map: &Map) -> (PolyLine, Distance) {
        let r = map.get_r(self.from);
        let dir = r.dir_and_offset(self.members[0].src).0;
        // Points away from the intersection
        let pl = if dir {
            r.center_pts.clone()
        } else {
            r.center_pts.reversed()
        };

        let mut offsets: Vec<usize> = self
            .members
            .iter()
            .map(|t| r.dir_and_offset(t.src).1)
            .collect();
        offsets.sort();
        offsets.dedup();
        // TODO This breaks if the group is non-contiguous. Add a rightmost bike lane that gets a
        // crazy left turn.
        let offset = if offsets.len() % 2 == 0 {
            // Middle of two lanes
            (offsets[offsets.len() / 2] as f64) - 0.5
        } else {
            offsets[offsets.len() / 2] as f64
        };
        let pl = pl.shift_right(LANE_THICKNESS * (0.5 + offset)).unwrap();
        let pl = if self.is_crosswalk
            && map.get_l(self.members[0].src).src_i == self.members[0].parent
        {
            pl
        } else {
            pl.reversed()
        };
        let width = LANE_THICKNESS * ((*offsets.last().unwrap() - offsets[0] + 1) as f64);
        (pl, width)
    }
}
