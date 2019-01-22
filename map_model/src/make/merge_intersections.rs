use crate::make::half_map::HalfMap;
use crate::{Intersection, IntersectionID, Lane, LaneID, Road, RoadID};
use abstutil::Timer;
use dimensioned::si;
use std::collections::{BTreeSet, HashMap, HashSet};
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
        if i - merged >= m.roads.len() {
            continue;
        }

        // When we delete road X, the entire list of roads shifts down, so on the next iteration,
        // we want to reconsider the new X.
        let r = &m.roads[i - merged];
        if r.center_pts.length() < MIN_ROAD_LENGTH
            && !m.intersections[r.src_i.0].is_dead_end()
            && !m.intersections[r.dst_i.0].is_dead_end()
        {
            m = merge(r.id, m);
            merged += 1;
        }
    }

    info!("Merged {} short roads", merged);
    m
}

fn merge(delete_r: RoadID, mut m: HalfMap) -> HalfMap {
    let old_i1 = m.roads[delete_r.0].src_i;
    let old_i2 = m.roads[delete_r.0].dst_i;
    // Note delete_r might be a loop.

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
        stable_id: m.intersections[old_i1.0].stable_id,
        incoming_lanes: Vec::new(),
        outgoing_lanes: Vec::new(),
        roads: BTreeSet::new(),
    });

    // For all of the connected roads and children lanes of the old intersections, fix up the
    // references to/from the intersection
    for old_i in &[old_i1, old_i2] {
        for r_id in m.intersections[old_i.0].roads.clone() {
            if r_id == delete_r {
                continue;
            }
            m.intersections[new_i.0].roads.insert(r_id);

            let r = &mut m.roads[r_id.0];
            if r.src_i == *old_i {
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
            }
            // This isn't an else, because r might be a loop.
            if r.dst_i == *old_i {
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

    delete.apply(m)
}

struct Deleter {
    roads: HashSet<RoadID>,
    lanes: HashSet<LaneID>,
    intersections: HashSet<IntersectionID>,
}

impl Deleter {
    fn new() -> Deleter {
        Deleter {
            roads: HashSet::new(),
            lanes: HashSet::new(),
            intersections: HashSet::new(),
        }
    }

    fn apply(self, mut m: HalfMap) -> HalfMap {
        assert!(m.turns.is_empty());

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
            assert!(i.turns.is_empty());
            for l in i.incoming_lanes.iter_mut() {
                *l = rename_lanes[l];
            }
            for l in i.outgoing_lanes.iter_mut() {
                *l = rename_lanes[l];
            }
            i.roads = i.roads.iter().map(|r| rename_roads[r]).collect();
        }

        m
    }
}
