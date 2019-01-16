use crate::{
    Intersection, IntersectionID, IntersectionType, Lane, LaneID, LaneType, Road, Turn, TurnID,
    TurnType, LANE_THICKNESS,
};
use abstutil::wraparound_get;
use dimensioned::si;
use geom::{Line, PolyLine, Pt2D};
use nbez::{Bez3o, BezCurve, Point2d};
use std::collections::{BTreeSet, HashSet};
use std::iter;

pub fn make_all_turns(i: &Intersection, roads: &Vec<&Road>, lanes: &Vec<&Lane>) -> Vec<Turn> {
    if i.intersection_type == IntersectionType::Border {
        return Vec::new();
    }

    let mut turns: Vec<Turn> = Vec::new();
    turns.extend(make_vehicle_turns(i, roads, lanes));
    turns.extend(make_walking_turns(i, roads, lanes));
    let turns = dedupe(turns);

    // Make sure every incoming lane has a turn originating from it, and every outgoing lane has a
    // turn leading to it. Except for parking lanes, of course.
    let mut incoming_missing: HashSet<LaneID> = HashSet::new();
    for l in &i.incoming_lanes {
        if lanes[l.0].lane_type != LaneType::Parking {
            incoming_missing.insert(*l);
        }
    }
    let mut outgoing_missing: HashSet<LaneID> = HashSet::new();
    for l in &i.outgoing_lanes {
        if lanes[l.0].lane_type != LaneType::Parking {
            outgoing_missing.insert(*l);
        }
    }
    for t in &turns {
        incoming_missing.remove(&t.id.src);
        outgoing_missing.remove(&t.id.dst);
    }
    if !incoming_missing.is_empty() || !outgoing_missing.is_empty() {
        // TODO Annoying, but this error is noisy for border nodes.
        error!(
            "Turns for {} orphan some lanes. Incoming: {:?}, outgoing: {:?}",
            i.id, incoming_missing, outgoing_missing
        );
    }

    turns
}

fn dedupe(turns: Vec<Turn>) -> Vec<Turn> {
    let mut ids = HashSet::new();
    let mut keep: Vec<Turn> = Vec::new();
    for t in turns.into_iter() {
        if ids.contains(&t.id) {
            panic!("Duplicate turns {}!", t.id);
        } else {
            ids.insert(t.id);
            keep.push(t);
        }
    }
    keep
}

fn make_vehicle_turns(i: &Intersection, all_roads: &Vec<&Road>, lanes: &Vec<&Lane>) -> Vec<Turn> {
    let roads: Vec<&Road> = i.roads.iter().map(|r| all_roads[r.0]).collect();
    let mut lane_types: BTreeSet<LaneType> = BTreeSet::new();
    for r in &roads {
        let (t1, t2) = r.get_lane_types();
        for lt in t1.into_iter().chain(t2.into_iter()) {
            lane_types.insert(lt);
        }
    }
    lane_types.remove(&LaneType::Parking);
    lane_types.remove(&LaneType::Sidewalk);

    let mut result = Vec::new();

    for lane_type in lane_types.into_iter() {
        if i.is_dead_end() {
            result.extend(make_vehicle_turns_for_dead_end(
                i, all_roads, lanes, lane_type,
            ));
            continue;
        }

        for r1 in &roads {
            // We can't filter incoming just on the preferred type, because we might be forced to
            // make a turn from a driving lane to a bike/bus lane.
            let incoming = filter_vehicle_lanes(r1.incoming_lanes(i.id), lane_type);
            if incoming.is_empty() {
                continue;
            }

            for r2 in &roads {
                if r1.id == r2.id {
                    continue;
                }
                let outgoing = filter_vehicle_lanes(r2.outgoing_lanes(i.id), lane_type);
                if outgoing.is_empty() {
                    continue;
                }

                // If we fell back to driving lanes for both incoming and outgoing and it's not
                // time, then skip. This should prevent duplicates.
                if lanes[incoming[0].0].lane_type != lane_type
                    && lanes[outgoing[0].0].lane_type != lane_type
                {
                    continue;
                }

                // Use an arbitrary lane from each road to get the angle between r1 and r2.
                let angle1 = lanes[incoming[0].0].last_line().angle();
                let angle2 = lanes[outgoing[0].0].first_line().angle();
                match TurnType::from_angles(angle1, angle2) {
                    TurnType::Straight => {
                        // Match up based on the relative number of lanes.
                        result.extend(match_up_lanes(lanes, i.id, &incoming, &outgoing));
                    }
                    TurnType::Right => {
                        result.push(make_vehicle_turn(
                            lanes,
                            i.id,
                            *incoming.last().unwrap(),
                            *outgoing.last().unwrap(),
                        ));
                    }
                    TurnType::Left => {
                        result.push(make_vehicle_turn(lanes, i.id, incoming[0], outgoing[0]));
                    }
                    _ => unreachable!(),
                };
            }
        }
    }

    result
}

fn match_up_lanes(
    lanes: &Vec<&Lane>,
    i: IntersectionID,
    incoming: &Vec<LaneID>,
    outgoing: &Vec<LaneID>,
) -> Vec<Turn> {
    let mut result = Vec::new();
    if incoming.len() < outgoing.len() {
        // Arbitrarily use the leftmost incoming lane to handle the excess.
        let padded_incoming: Vec<&LaneID> = iter::repeat(&incoming[0])
            .take(outgoing.len() - incoming.len())
            .chain(incoming.iter())
            .collect();
        assert_eq!(padded_incoming.len(), outgoing.len());
        for (l1, l2) in padded_incoming.iter().zip(outgoing.iter()) {
            result.push(make_vehicle_turn(lanes, i, **l1, *l2));
        }
    } else if incoming.len() > outgoing.len() {
        // TODO For non-dead-ends: Ideally if the left/rightmost lanes are for turning, use the
        // unused one to go straight.
        // But for now, arbitrarily use the leftmost outgoing road to handle the excess.
        let padded_outgoing: Vec<&LaneID> = iter::repeat(&outgoing[0])
            .take(incoming.len() - outgoing.len())
            .chain(outgoing.iter())
            .collect();
        assert_eq!(padded_outgoing.len(), incoming.len());
        for (l1, l2) in incoming.iter().zip(&padded_outgoing) {
            result.push(make_vehicle_turn(lanes, i, *l1, **l2));
        }
    } else {
        // The easy case!
        for (l1, l2) in incoming.iter().zip(outgoing.iter()) {
            result.push(make_vehicle_turn(lanes, i, *l1, *l2));
        }
    }
    result
}

fn make_vehicle_turns_for_dead_end(
    i: &Intersection,
    roads: &Vec<&Road>,
    lanes: &Vec<&Lane>,
    lane_type: LaneType,
) -> Vec<Turn> {
    let road = roads[i.roads.iter().next().unwrap().0];
    let incoming = filter_vehicle_lanes(road.incoming_lanes(i.id), lane_type);
    let outgoing = filter_vehicle_lanes(road.outgoing_lanes(i.id), lane_type);
    if incoming.is_empty() || outgoing.is_empty() {
        error!("{} needs to be a border node!", i.id);
        return Vec::new();
    }

    match_up_lanes(lanes, i.id, &incoming, &outgoing)
}

fn make_walking_turns(i: &Intersection, all_roads: &Vec<&Road>, lanes: &Vec<&Lane>) -> Vec<Turn> {
    // Sort roads by the angle into the intersection, so we can reason about sidewalks of adjacent
    // roads.
    let mut roads: Vec<&Road> = i.roads.iter().map(|id| all_roads[id.0]).collect();
    roads.sort_by_key(|r| {
        if r.src_i == i.id {
            r.center_pts
                .reversed()
                .last_line()
                .angle()
                .normalized_degrees() as i64
        } else if r.dst_i == i.id {
            r.center_pts.last_line().angle().normalized_degrees() as i64
        } else {
            panic!(
                "Incident road {} doesn't have an endpoint at {}",
                r.id, i.id
            );
        }
    });

    let mut result: Vec<Turn> = Vec::new();

    for idx1 in 0..roads.len() {
        if let Some(l1) = get_sidewalk(lanes, roads[idx1].incoming_lanes(i.id)) {
            // Make the crosswalk to the other side
            if let Some(l2) = get_sidewalk(lanes, roads[idx1].outgoing_lanes(i.id)) {
                result.extend(make_crosswalks(i.id, l1, l2));
            }

            // Find the shared corner
            if roads.len() > 1 {
                // TODO -1 and not +1 is brittle... must be the angle sorting
                if let Some(l2) = get_sidewalk(
                    lanes,
                    wraparound_get(&roads, (idx1 as isize) - 1).outgoing_lanes(i.id),
                ) {
                    result.push(Turn {
                        id: turn_id(i.id, l1.id, l2.id),
                        turn_type: TurnType::SharedSidewalkCorner,
                        geom: PolyLine::new(vec![l1.last_pt(), l2.first_pt()]),
                        lookup_idx: 0,
                    });
                    result.push(Turn {
                        id: turn_id(i.id, l2.id, l1.id),
                        turn_type: TurnType::SharedSidewalkCorner,
                        geom: PolyLine::new(vec![l2.first_pt(), l1.last_pt()]),
                        lookup_idx: 0,
                    });
                } else if roads.len() > 2 {
                    // See if we need to add a crosswalk over this adjacent road.
                    // TODO This is brittle; I could imagine having to cross two adjacent highway
                    // ramps to get to the next sidewalk.
                    if let Some(l2) = get_sidewalk(
                        lanes,
                        wraparound_get(&roads, (idx1 as isize) - 2).outgoing_lanes(i.id),
                    ) {
                        result.extend(make_crosswalks(i.id, l1, l2));
                    }
                }
            }
        }
    }

    result
}

fn make_crosswalks(i: IntersectionID, l1: &Lane, l2: &Lane) -> Vec<Turn> {
    // Jut out a bit into the intersection, cross over, then jut back in.
    let line = Line::new(l1.last_pt(), l2.first_pt()).shift_right(LANE_THICKNESS / 2.0);
    let geom_fwds = PolyLine::new(vec![l1.last_pt(), line.pt1(), line.pt2(), l2.first_pt()]);

    vec![
        Turn {
            id: turn_id(i, l1.id, l2.id),
            turn_type: TurnType::Crosswalk,
            geom: geom_fwds.clone(),
            lookup_idx: 0,
        },
        Turn {
            id: turn_id(i, l2.id, l1.id),
            turn_type: TurnType::Crosswalk,
            geom: geom_fwds.reversed(),
            lookup_idx: 0,
        },
    ]
}

fn turn_id(parent: IntersectionID, src: LaneID, dst: LaneID) -> TurnID {
    TurnID { parent, src, dst }
}

fn get_sidewalk<'a>(lanes: &'a Vec<&Lane>, children: &Vec<(LaneID, LaneType)>) -> Option<&'a Lane> {
    for (id, lt) in children {
        if *lt == LaneType::Sidewalk {
            return Some(lanes[id.0]);
        }
    }
    None
}

fn filter_vehicle_lanes(lanes: &Vec<(LaneID, LaneType)>, preferred: LaneType) -> Vec<LaneID> {
    let preferred = filter_lanes(lanes, preferred);
    if !preferred.is_empty() {
        return preferred;
    }
    filter_lanes(lanes, LaneType::Driving)
}

fn filter_lanes(lanes: &Vec<(LaneID, LaneType)>, filter: LaneType) -> Vec<LaneID> {
    lanes
        .iter()
        .filter_map(|(id, lt)| if *lt == filter { Some(*id) } else { None })
        .collect()
}

fn make_vehicle_turn(lanes: &Vec<&Lane>, i: IntersectionID, l1: LaneID, l2: LaneID) -> Turn {
    let src = lanes[l1.0];
    let dst = lanes[l2.0];
    let turn_type = TurnType::from_angles(src.last_line().angle(), dst.first_line().angle());

    let geom = if turn_type == TurnType::Straight {
        PolyLine::new(vec![src.last_pt(), dst.first_pt()])
    } else {
        // The control points are straight out/in from the source/destination lanes, so
        // that the car exits and enters at the same angle as the road.
        let src_line = src.last_line();
        let dst_line = dst.first_line().reverse();

        // TODO Tune the 5.0 and pieces
        let curve = Bez3o::new(
            to_pt(src.last_pt()),
            to_pt(src_line.unbounded_dist_along(src_line.length() + 5.0 * si::M)),
            to_pt(dst_line.unbounded_dist_along(dst_line.length() + 5.0 * si::M)),
            to_pt(dst.first_pt()),
        );
        let pieces = 5;
        PolyLine::new(
            (0..=pieces)
                .map(|i| {
                    from_pt(
                        curve
                            .interp(1.0 / f64::from(pieces) * f64::from(i))
                            .unwrap(),
                    )
                })
                .collect(),
        )
    };

    Turn {
        id: turn_id(i, l1, l2),
        turn_type,
        geom,
        lookup_idx: 0,
    }
}

fn to_pt(pt: Pt2D) -> Point2d<f64> {
    Point2d::new(pt.x(), pt.y())
}

fn from_pt(pt: Point2d<f64>) -> Pt2D {
    Pt2D::new(pt.x, pt.y)
}
