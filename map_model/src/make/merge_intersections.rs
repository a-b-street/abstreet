use crate::make::half_map::HalfMap;
use crate::{Intersection, IntersectionID, Lane, LaneID, Road, RoadID, Turn, TurnID};
use abstutil::Timer;
use dimensioned::si;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::marker;

const MIN_ROAD_LENGTH: si::Meter<f64> = si::Meter {
    value_unsafe: 15.0,
    _marker: marker::PhantomData,
};

pub fn merge_intersections(mut m: HalfMap, timer: &mut Timer) -> HalfMap {
    timer.start_iter("merge short roads", m.roads.len());

    let mut merged = 0;
    for i in 0..m.roads.len() {
        timer.next();
        // We destroy roads and shorten this list as we go. Don't break, so the timer finishes.
        if i >= m.roads.len() {
            continue;
        }

        let r = &m.roads[i];
        if r.center_pts.length() < MIN_ROAD_LENGTH
            && !m.intersections[r.src_i.0].is_dead_end()
            && !m.intersections[r.dst_i.0].is_dead_end()
        {
            m = merge(RoadID(i), m);
            merged += 1;
        }
    }

    info!("Merged {} short roads", merged);

    m
}

fn merge(delete_r: RoadID, mut m: HalfMap) -> HalfMap {
    let old_i1 = m.roads[delete_r.0].src_i;
    let old_i2 = m.roads[delete_r.0].dst_i;

    let mut delete = Deleter::new();

    // Delete the road
    delete.roads.insert(delete_r);
    // Delete all of its children lanes
    for (id, _) in &m.roads[delete_r.0].children_forwards {
        delete.lanes.insert(*id);
    }
    for (id, _) in &m.roads[delete_r.0].children_backwards {
        delete.lanes.insert(*id);
    }
    // Delete the two connected intersections
    delete.intersections.insert(old_i1);
    delete.intersections.insert(old_i2);
    // Delete all of the turns from the two intersections
    delete.turns.extend(m.intersections[old_i1.0].turns.clone());
    delete.turns.extend(m.intersections[old_i2.0].turns.clone());

    // Make a new intersection to replace the two old ones
    // TODO Arbitrarily take point, elevation, type, label from one of the old intersections.
    let new_i = IntersectionID(m.intersections.len());
    m.intersections.push(Intersection {
        id: new_i,
        point: m.intersections[old_i1.0].point,
        polygon: Vec::new(),
        turns: Vec::new(),
        elevation: m.intersections[old_i1.0].elevation,
        intersection_type: m.intersections[old_i1.0].intersection_type,
        label: m.intersections[old_i1.0].label.clone(),
        incoming_lanes: Vec::new(),
        outgoing_lanes: Vec::new(),
        roads: BTreeSet::new(),
    });

    // For all of the connected roads and children lanes of the old intersections, fix up the
    // references to/from the intersection
    for old_i in vec![old_i1, old_i2] {
        for r_id in m.intersections[old_i.0].roads.clone() {
            if r_id == delete_r {
                continue;
            }
            m.intersections[new_i.0].roads.insert(r_id);

            let r = &mut m.roads[r_id.0];
            if r.src_i == old_i {
                // Outgoing from old_i
                r.src_i = new_i;
                for (l, _) in &r.children_forwards {
                    m.lanes[l.0].src_i = new_i;
                    m.intersections[new_i.0].outgoing_lanes.push(*l);
                }
                for (l, _) in &r.children_backwards {
                    m.lanes[l.0].dst_i = new_i;
                    m.intersections[new_i.0].incoming_lanes.push(*l);
                }
            } else {
                assert_eq!(r.dst_i, old_i);
                // Incoming to old_i
                r.dst_i = new_i;
                for (l, _) in &r.children_backwards {
                    m.lanes[l.0].src_i = new_i;
                    m.intersections[new_i.0].outgoing_lanes.push(*l);
                }
                for (l, _) in &r.children_forwards {
                    m.lanes[l.0].dst_i = new_i;
                    m.intersections[new_i.0].incoming_lanes.push(*l);
                }
            }
        }
    }

    // Populate the intersection with turns constructed from the old turns
    for old_i in vec![old_i1, old_i2] {
        for id in m.intersections[old_i.0].turns.clone() {
            let orig_turn = &m.turns[&id];

            // Skip turns starting in the middle of the intersection.
            if delete.lanes.contains(&orig_turn.id.src) {
                continue;
            }

            let mut new_turn = orig_turn.clone();
            new_turn.id.parent = new_i;

            // The original turn never crossed the deleted road. Preserve its geometry.
            // TODO But what if the intersection polygon changed and made lane trimmed lines change
            // and so the turn geometry should change?
            if !delete.lanes.contains(&new_turn.id.dst) {
                m.intersections[new_i.0].turns.push(new_turn.id);
                m.turns.insert(new_turn.id, new_turn);
                continue;
            }

            if new_turn.between_sidewalks() {
                // TODO Handle this. Gets weird because of bidirectionality.
                continue;
            }

            // Make new composite turns! All of them will include the deleted lane's geometry.
            new_turn
                .geom
                .extend(m.lanes[new_turn.id.dst.0].lane_center_pts.clone());

            let other_old_i = if old_i == old_i1 { old_i2 } else { old_i1 };
            for t in m.intersections[other_old_i.0].turns.clone() {
                if t.src != new_turn.id.dst {
                    continue;
                }
                let mut composite_turn = new_turn.clone();
                composite_turn.id.dst = t.dst;
                composite_turn.geom.extend(m.turns[&t].geom.clone());
                // TODO Fiddle with turn_type
                m.intersections[new_i.0].turns.push(composite_turn.id);
                m.turns.insert(composite_turn.id, composite_turn);
            }
        }
    }

    delete.apply(m)
}

struct Deleter {
    roads: HashSet<RoadID>,
    lanes: HashSet<LaneID>,
    intersections: HashSet<IntersectionID>,
    turns: HashSet<TurnID>,
}

impl Deleter {
    fn new() -> Deleter {
        Deleter {
            roads: HashSet::new(),
            lanes: HashSet::new(),
            intersections: HashSet::new(),
            turns: HashSet::new(),
        }
    }

    fn apply(self, mut m: HalfMap) -> HalfMap {
        // Actually delete and compact stuff...
        for t in self.turns {
            m.turns.remove(&t);
        }

        let mut rename_roads: HashMap<RoadID, RoadID> = HashMap::new();
        let mut keep_roads: Vec<Road> = Vec::new();
        for r in m.roads.drain(0..) {
            if self.roads.contains(&r.id) {
                continue;
            }
            rename_roads.insert(r.id, RoadID(keep_roads.len()));
            keep_roads.push(r);
        }
        m.roads = keep_roads;

        let mut rename_lanes: HashMap<LaneID, LaneID> = HashMap::new();
        let mut keep_lanes: Vec<Lane> = Vec::new();
        for l in m.lanes.drain(0..) {
            if self.lanes.contains(&l.id) {
                continue;
            }
            rename_lanes.insert(l.id, LaneID(keep_lanes.len()));
            keep_lanes.push(l);
        }
        m.lanes = keep_lanes;

        let mut rename_intersections: HashMap<IntersectionID, IntersectionID> = HashMap::new();
        let mut keep_intersections: Vec<Intersection> = Vec::new();
        for i in m.intersections.drain(0..) {
            if self.intersections.contains(&i.id) {
                continue;
            }
            rename_intersections.insert(i.id, IntersectionID(keep_intersections.len()));
            keep_intersections.push(i);
        }
        m.intersections = keep_intersections;

        // Fix up IDs everywhere
        for r in m.roads.iter_mut() {
            r.id = rename_roads[&r.id];
            for (l, _) in r.children_forwards.iter_mut() {
                *l = rename_lanes[l];
            }
            for (l, _) in r.children_backwards.iter_mut() {
                *l = rename_lanes[l];
            }
            r.src_i = rename_intersections[&r.src_i];
            r.dst_i = rename_intersections[&r.dst_i];
        }
        for l in m.lanes.iter_mut() {
            l.id = rename_lanes[&l.id];
            l.parent = rename_roads[&l.parent];
            l.src_i = rename_intersections[&l.src_i];
            l.dst_i = rename_intersections[&l.dst_i];
        }
        for i in m.intersections.iter_mut() {
            i.id = rename_intersections[&i.id];
            for t in i.turns.iter_mut() {
                t.parent = rename_intersections[&t.parent];
                t.src = rename_lanes[&t.src];
                t.dst = rename_lanes[&t.dst];
            }
            for l in i.incoming_lanes.iter_mut() {
                *l = rename_lanes[l];
            }
            for l in i.outgoing_lanes.iter_mut() {
                *l = rename_lanes[l];
            }
            i.roads = i.roads.iter().map(|r| rename_roads[r]).collect();
        }
        let mut new_turns: BTreeMap<TurnID, Turn> = BTreeMap::new();
        for (_, mut t) in m.turns.into_iter() {
            let id = TurnID {
                parent: rename_intersections[&t.id.parent],
                src: rename_lanes[&t.id.src],
                dst: rename_lanes[&t.id.dst],
            };
            t.id = id;
            new_turns.insert(t.id, t);
        }
        m.turns = new_turns;

        m
    }
}
