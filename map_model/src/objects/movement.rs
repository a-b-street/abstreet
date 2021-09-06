use std::collections::{BTreeMap, BTreeSet};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use abstutil::MultiMap;
use geom::{Angle, Distance, PolyLine, Pt2D};

use crate::{DirectedRoadID, Direction, IntersectionID, Map, TurnID, TurnType};

/// A movement is like a turn, but with less detail -- it identifies a movement from one directed
/// road to another.
/// One road usually has 4 crosswalks, each a singleton Movement. We need all of the information
/// here to keep each crosswalk separate.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct MovementID {
    pub from: DirectedRoadID,
    pub to: DirectedRoadID,
    pub parent: IntersectionID,
    pub crosswalk: bool,
}

/// This is cheaper to store than a MovementID. It simply indexes into the list of movements.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CompressedMovementID {
    pub i: IntersectionID,
    // There better not be any intersection with more than 256 movements...
    pub idx: u8,
}

/// A Movement groups all turns from one road to another, letting traffic signals and pathfinding
/// operate at a higher level of abstraction.
// TODO Unclear how this plays with different lane types
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Movement {
    pub id: MovementID,
    pub turn_type: TurnType,
    pub members: Vec<TurnID>,
    /// The "overall" path of movement, aka, an "average" of the turn geometry
    pub geom: PolyLine,
    pub angle: Angle,
}

impl Movement {
    pub(crate) fn for_i(i: IntersectionID, map: &Map) -> BTreeMap<MovementID, Movement> {
        let mut results = BTreeMap::new();
        let mut movements: MultiMap<(DirectedRoadID, DirectedRoadID), TurnID> = MultiMap::new();
        for turn in &map.get_i(i).turns {
            let from = map.get_l(turn.id.src).get_directed_parent();
            let to = map.get_l(turn.id.dst).get_directed_parent();
            match turn.turn_type {
                TurnType::SharedSidewalkCorner => {}
                TurnType::Crosswalk => {
                    let id = MovementID {
                        from,
                        to,
                        parent: i,
                        crosswalk: true,
                    };
                    results.insert(
                        id,
                        Movement {
                            id,
                            turn_type: TurnType::Crosswalk,
                            members: vec![turn.id],
                            geom: turn.geom.clone(),
                            angle: turn.angle(),
                        },
                    );
                }
                _ => {
                    movements.insert((from, to), turn.id);
                }
            }
        }
        for ((from, to), members) in movements.consume() {
            let geom = match movement_geom(
                members.iter().map(|t| &map.get_t(*t).geom).collect(),
                from,
                to,
            ) {
                Ok(geom) => geom,
                Err(err) => {
                    warn!("Weird movement geometry at {}: {}", i, err);
                    // Just use one of the turns
                    map.get_t(*members.iter().next().unwrap()).geom.clone()
                }
            };
            let turn_types: BTreeSet<TurnType> =
                members.iter().map(|t| map.get_t(*t).turn_type).collect();
            if turn_types.len() > 1 {
                warn!(
                    "Movement between {} and {} has weird turn types! {:?}",
                    from, to, turn_types
                );
            }
            let members: Vec<TurnID> = members.into_iter().collect();
            let id = MovementID {
                from,
                to,
                parent: i,
                crosswalk: false,
            };
            results.insert(
                id,
                Movement {
                    id,
                    turn_type: *turn_types.iter().next().unwrap(),
                    angle: map.get_t(members[0]).angle(),
                    members,
                    geom,
                },
            );
        }
        // The result might be empty for border intersections; that's fine
        results
    }

    /// Polyline points FROM intersection
    pub fn src_center_and_width(&self, map: &Map) -> (PolyLine, Distance) {
        let r = map.get_r(self.id.from.id);

        let mut leftmost = Distance::meters(99999.0);
        let mut rightmost = Distance::ZERO;
        let mut left = Distance::ZERO;

        for l in &r.lanes {
            let right = left + l.width;

            if self.members.iter().any(|t| t.src == l.id) {
                leftmost = leftmost.min(left);
                rightmost = rightmost.max(right);
            }

            left = right;
        }

        let mut pl = r
            .get_left_side()
            .must_shift_right((leftmost + rightmost) / 2.0);
        if self.id.from.dir == Direction::Back {
            pl = pl.reversed();
        }
        // Flip direction, so we point away from the intersection
        if !self.id.crosswalk || map.get_l(self.members[0].src).src_i != self.members[0].parent {
            pl = pl.reversed()
        };
        (pl, rightmost - leftmost)
    }

    pub fn conflicts_with(&self, other: &Movement) -> bool {
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
        // TODO If you hit a panic below, you've probably got two separate roads overlapping.
        // Fix it in OSM. Examples: https://www.openstreetmap.org/changeset/87465499,
        // https://www.openstreetmap.org/changeset/85952811
        /*if self.geom == other.geom {
            println!("*********** {:?} and {:?} have the same geom", self.id, other.id);
            return true;
        }*/
        self.geom.intersection(&other.geom).is_some()
    }
}

fn movement_geom(
    polylines: Vec<&PolyLine>,
    from: DirectedRoadID,
    to: DirectedRoadID,
) -> Result<PolyLine> {
    let num_pts = polylines[0].points().len();
    for pl in &polylines {
        if num_pts != pl.points().len() {
            // Kiiiiinda spammy
            if false {
                warn!(
                    "Movement between {} and {} can't make nice geometry",
                    from, to
                );
            }
            return Ok(polylines[0].clone());
        }
    }

    let mut pts = Vec::new();
    for idx in 0..num_pts {
        pts.push(Pt2D::center(
            &polylines
                .iter()
                .map(|pl| pl.points()[idx])
                .collect::<Vec<_>>(),
        ));
    }
    PolyLine::deduping_new(pts)
}
