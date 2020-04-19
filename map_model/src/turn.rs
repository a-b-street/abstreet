use crate::{DirectedRoadID, IntersectionID, LaneID, Map};
use abstutil::MultiMap;
use geom::{Angle, Distance, PolyLine, Pt2D};
use serde_derive::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
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

// TODO This concept may be dated, now that TurnGroups exist. Within a group, the lane-changing
// turns should be treated as less important.
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, Copy, PartialOrd)]
pub enum TurnPriority {
    // For stop signs: Can't currently specify this!
    // For traffic signals: Can't do this turn right now.
    Banned,
    // For stop signs: cars have to stop before doing this turn, and are accepted with the lowest
    // priority. For traffic signals: Cars can do this immediately if there are no previously
    // accepted conflicting turns.
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
    // Empty except for TurnType::Crosswalk. Usually just one other ID, except for the case of 4
    // duplicates at a degenerate intersection.
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
}

// One road usually has 4 crosswalks, each a singleton TurnGroup. We need all of the information
// here to keep each crosswalk separate.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TurnGroupID {
    pub from: DirectedRoadID,
    pub to: DirectedRoadID,
    pub parent: IntersectionID,
    pub crosswalk: bool,
}

// TODO Unclear how this plays with different lane types
// This is only useful for traffic signals currently.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct TurnGroup {
    pub id: TurnGroupID,
    pub turn_type: TurnType,
    pub members: Vec<TurnID>,
    // The "overall" path of movement, aka, an "average" of the turn geometry
    pub geom: PolyLine,
    pub angle: Angle,
}

impl TurnGroup {
    pub(crate) fn for_i(i: IntersectionID, map: &Map) -> BTreeMap<TurnGroupID, TurnGroup> {
        let mut results = BTreeMap::new();
        let mut groups: MultiMap<(DirectedRoadID, DirectedRoadID), TurnID> = MultiMap::new();
        for turn in map.get_turns_in_intersection(i) {
            let from = map.get_l(turn.id.src).get_directed_parent(map);
            let to = map.get_l(turn.id.dst).get_directed_parent(map);
            match turn.turn_type {
                TurnType::SharedSidewalkCorner => {}
                TurnType::Crosswalk => {
                    let id = TurnGroupID {
                        from,
                        to,
                        parent: i,
                        crosswalk: true,
                    };
                    results.insert(
                        id,
                        TurnGroup {
                            id,
                            turn_type: TurnType::Crosswalk,
                            members: vec![turn.id],
                            geom: turn.geom.clone(),
                            angle: turn.angle(),
                        },
                    );
                }
                _ => {
                    groups.insert((from, to), turn.id);
                }
            }
        }
        for ((from, to), members) in groups.consume() {
            let geom = turn_group_geom(
                members.iter().map(|t| &map.get_t(*t).geom).collect(),
                from,
                to,
            );
            let turn_types: BTreeSet<TurnType> = members
                .iter()
                .map(|t| match map.get_t(*t).turn_type {
                    TurnType::Crosswalk | TurnType::SharedSidewalkCorner => unreachable!(),
                    TurnType::Straight | TurnType::LaneChangeLeft | TurnType::LaneChangeRight => {
                        TurnType::Straight
                    }
                    TurnType::Left => TurnType::Left,
                    TurnType::Right => TurnType::Right,
                })
                .collect();
            if turn_types.len() > 1 {
                println!(
                    "TurnGroup between {} and {} has weird turn types! {:?}",
                    from, to, turn_types
                );
            }
            let members: Vec<TurnID> = members.into_iter().collect();
            let id = TurnGroupID {
                from,
                to,
                parent: i,
                crosswalk: false,
            };
            results.insert(
                id,
                TurnGroup {
                    id,
                    turn_type: *turn_types.iter().next().unwrap(),
                    angle: map.get_t(members[0]).angle(),
                    members,
                    geom,
                },
            );
        }
        if results.is_empty() {
            panic!("{} has no TurnGroups!", i);
        }
        results
    }

    // Polyline points FROM intersection
    pub fn src_center_and_width(&self, map: &Map) -> (PolyLine, Distance) {
        let r = map.get_r(self.id.from.id);
        let dir = self.id.from.forwards;
        // Points towards the intersection
        let pl = if dir {
            r.get_current_center(map)
        } else {
            r.get_current_center(map).reversed()
        };

        // TODO Poorly expressed. We just want the first leftmost value, and the last rightmost.
        let mut leftmost = Distance::meters(99999.0);
        let mut rightmost = Distance::ZERO;
        let mut left = Distance::ZERO;
        let mut right = Distance::ZERO;

        for l in r.lanes_on_side(dir) {
            right += map.get_l(l).width;

            if self.members.iter().any(|t| t.src == l) {
                leftmost = leftmost.min(left);
                rightmost = rightmost.max(right);
            }

            left += map.get_l(l).width;
        }

        let pl = map.right_shift(pl, (leftmost + rightmost) / 2.0).unwrap();
        // Flip direction, so we point away from the intersection
        let pl = if self.id.crosswalk
            && map.get_l(self.members[0].src).src_i == self.members[0].parent
        {
            pl
        } else {
            pl.reversed()
        };
        (pl, rightmost - leftmost)
    }

    pub fn conflicts_with(&self, other: &TurnGroup) -> bool {
        if self.id == other.id {
            return false;
        }
        if self.turn_type == TurnType::Crosswalk && other.turn_type == TurnType::Crosswalk {
            return false;
        }

        if self.id.from == other.id.from
            && self.turn_type != TurnType::Crosswalk
            && other.turn_type != TurnType::Crosswalk
        {
            return false;
        }
        if self.id.to == other.id.to
            && self.turn_type != TurnType::Crosswalk
            && other.turn_type != TurnType::Crosswalk
        {
            return true;
        }
        self.geom.intersection(&other.geom).is_some()
    }
}

fn turn_group_geom(
    polylines: Vec<&PolyLine>,
    from: DirectedRoadID,
    to: DirectedRoadID,
) -> PolyLine {
    let num_pts = polylines[0].points().len();
    for pl in &polylines {
        if num_pts != pl.points().len() {
            println!(
                "TurnGroup between {} and {} can't make nice geometry",
                from, to
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
