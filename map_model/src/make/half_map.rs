use crate::make;
use crate::{
    raw_data, Intersection, IntersectionID, IntersectionType, Lane, LaneID, MapEdits, Road, RoadID,
    Turn, TurnID, LANE_THICKNESS,
};
use abstutil::Timer;
use geom::{GPSBounds, HashablePt2D, PolyLine, Pt2D};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

pub struct HalfMap {
    pub roads: Vec<Road>,
    pub lanes: Vec<Lane>,
    pub intersections: Vec<Intersection>,
    pub turns: BTreeMap<TurnID, Turn>,
}

pub fn make_half_map(
    data: &raw_data::Map,
    gps_bounds: &GPSBounds,
    edits: &MapEdits,
    timer: &mut Timer,
) -> HalfMap {
    let mut m = HalfMap {
        roads: Vec::new(),
        lanes: Vec::new(),
        intersections: Vec::new(),
        turns: BTreeMap::new(),
    };

    let mut pt_to_intersection: HashMap<HashablePt2D, IntersectionID> = HashMap::new();

    for (idx, i) in data.intersections.iter().enumerate() {
        let id = IntersectionID(idx);
        let pt = Pt2D::from_gps(i.point, &gps_bounds).unwrap();
        m.intersections.push(Intersection {
            id,
            point: pt,
            polygon: Vec::new(),
            turns: Vec::new(),
            elevation: i.elevation,
            // Might change later
            intersection_type: i.intersection_type,
            label: i.label.clone(),
            incoming_lanes: Vec::new(),
            outgoing_lanes: Vec::new(),
            roads: BTreeSet::new(),
        });
        pt_to_intersection.insert(HashablePt2D::from(pt), id);
    }

    let mut counter = 0;
    timer.start_iter("expand roads to lanes", data.roads.len());
    for (_, r) in data.roads.iter().enumerate() {
        timer.next();
        let road_id = RoadID(m.roads.len());
        let road_center_pts = PolyLine::new(
            r.points
                .iter()
                .map(|coord| Pt2D::from_gps(*coord, &gps_bounds).unwrap())
                .collect(),
        );
        let i1 = pt_to_intersection[&HashablePt2D::from(road_center_pts.first_pt())];
        let i2 = pt_to_intersection[&HashablePt2D::from(road_center_pts.last_pt())];

        if i1 == i2 {
            // TODO Cul-de-sacs should be valid, but it really makes pathfinding screwy
            error!(
                "OSM way {} is a loop on {}, skipping what would've been {}",
                r.osm_way_id, i1, road_id
            );
            continue;
        }

        m.roads.push(Road {
            id: road_id,
            osm_tags: r.osm_tags.clone(),
            osm_way_id: r.osm_way_id,
            children_forwards: Vec::new(),
            children_backwards: Vec::new(),
            center_pts: road_center_pts.clone(),
            src_i: i1,
            dst_i: i2,
        });

        // TODO move this to make/lanes.rs too
        for lane in make::lanes::get_lane_specs(r, road_id, edits) {
            let id = LaneID(counter);
            counter += 1;

            let mut unshifted_pts = road_center_pts.clone();
            if lane.reverse_pts {
                unshifted_pts = unshifted_pts.reversed();
            }
            let (src_i, dst_i) = if lane.reverse_pts { (i2, i1) } else { (i1, i2) };
            m.intersections[src_i.0].outgoing_lanes.push(id);
            m.intersections[src_i.0].roads.insert(road_id);
            m.intersections[dst_i.0].incoming_lanes.push(id);
            m.intersections[dst_i.0].roads.insert(road_id);

            // TODO probably different behavior for oneways
            // TODO need to factor in yellow center lines (but what's the right thing to even do?
            // Reverse points for British-style driving on the left
            let width = LANE_THICKNESS * (0.5 + f64::from(lane.offset));
            let (lane_center_pts, probably_broken) = match unshifted_pts.shift(width) {
                Some(pts) => (pts, false),
                // TODO wasteful to calculate again, but eh
                None => (unshifted_pts.shift_blindly(width), true),
            };

            // lane_center_pts will get updated in the next pass
            m.lanes.push(Lane {
                id,
                lane_center_pts,
                probably_broken,
                src_i,
                dst_i,
                lane_type: lane.lane_type,
                parent: road_id,
                building_paths: Vec::new(),
                bus_stops: Vec::new(),
            });
            if lane.reverse_pts {
                m.roads[road_id.0]
                    .children_backwards
                    .push((id, lane.lane_type));
            } else {
                m.roads[road_id.0]
                    .children_forwards
                    .push((id, lane.lane_type));
            }
        }
    }

    for i in m.intersections.iter_mut() {
        // Is the intersection a border?
        if is_border(i, &m.lanes) {
            i.intersection_type = IntersectionType::Border;
        }
    }

    timer.start_iter("find each intersection polygon", m.intersections.len());
    for i in m.intersections.iter_mut() {
        timer.next();

        if i.incoming_lanes.is_empty() && i.outgoing_lanes.is_empty() {
            panic!("{:?} is orphaned!", i);
        }

        i.polygon = make::intersections::initial_intersection_polygon(i, &m.roads);
    }

    timer.start_iter("trim lanes at each intersection", m.intersections.len());
    for i in &m.intersections {
        timer.next();
        make::trim_lines::trim_lines(&mut m.lanes, i);
    }

    for i in m.intersections.iter_mut() {
        for t in
            make::turns::make_all_turns(i, &m.roads.iter().collect(), &m.lanes.iter().collect())
        {
            assert!(!m.turns.contains_key(&t.id));
            i.turns.push(t.id);
            m.turns.insert(t.id, t);
        }
    }

    m = merge_intersections(m);

    // Recalculate all intersection polygons again, using the lanes' "teeth" this time.
    for i in m.intersections.iter_mut() {
        if i.incoming_lanes.is_empty() && i.outgoing_lanes.is_empty() {
            panic!("{:?} is orphaned!", i);
        }

        i.polygon = make::intersections::toothy_intersection_polygon(i, &m.lanes);
    }

    m
}

fn is_border(intersection: &Intersection, lanes: &Vec<Lane>) -> bool {
    // Raw data said it is.
    if intersection.intersection_type == IntersectionType::Border {
        if !intersection.is_dead_end() {
            panic!(
                "{:?} isn't a dead-end, but raw data said it's a border node",
                intersection
            );
        }
        return true;
    }
    // Bias for driving
    if !intersection.is_dead_end() {
        return false;
    }
    let has_driving_in = intersection
        .incoming_lanes
        .iter()
        .any(|l| lanes[l.0].is_driving());
    let has_driving_out = intersection
        .outgoing_lanes
        .iter()
        .any(|l| lanes[l.0].is_driving());
    has_driving_in != has_driving_out
}

fn merge_intersections(mut m: HalfMap) -> HalfMap {
    //m = merge_intersection(RoadID(428), m);
    //m = merge_intersection(RoadID(422), m);

    m = merge_intersection(RoadID(422), m);
    m = merge_intersection(RoadID(427), m);

    m
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

fn merge_intersection(delete_r: RoadID, mut m: HalfMap) -> HalfMap {
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
