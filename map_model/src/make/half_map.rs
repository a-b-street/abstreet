use crate::make;
use crate::{
    raw_data, Intersection, IntersectionID, IntersectionType, Lane, LaneID, MapEdits, Road, RoadID,
    Turn, TurnID, LANE_THICKNESS,
};
use abstutil::Timer;
use geom::{GPSBounds, HashablePt2D, PolyLine, Pt2D};
use std::collections::{BTreeMap, BTreeSet, HashMap};

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

            let (src_i, dst_i) = if lane.reverse_pts { (i2, i1) } else { (i1, i2) };
            m.intersections[src_i.0].outgoing_lanes.push(id);
            m.intersections[src_i.0].roads.insert(road_id);
            m.intersections[dst_i.0].incoming_lanes.push(id);
            m.intersections[dst_i.0].roads.insert(road_id);

            m.lanes.push(Lane {
                id,
                // Temporary dummy value; this'll be calculated a bit later.
                lane_center_pts: road_center_pts.clone(),
                probably_broken: false,
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

        i.polygon = make::intersections::initial_intersection_polygon(i, &mut m.roads);
    }

    timer.start_iter("make lane geometry", m.lanes.len());
    for l in m.lanes.iter_mut() {
        timer.next();

        let parent = &m.roads[l.parent.0];
        let (dir, offset) = parent.dir_and_offset(l.id);
        let unshifted_pts = if dir {
            parent.center_pts.clone()
        } else {
            parent.center_pts.reversed()
        };

        // TODO probably different behavior for oneways
        // TODO need to factor in yellow center lines (but what's the right thing to even do?
        // Reverse points for British-style driving on the left
        let width = LANE_THICKNESS * (0.5 + (offset as f64));
        let (lane_center_pts, probably_broken) = match unshifted_pts.shift(width) {
            Some(pts) => (pts, false),
            // TODO wasteful to calculate again, but eh
            None => (unshifted_pts.shift_blindly(width), true),
        };
        l.lane_center_pts = lane_center_pts;
        l.probably_broken = probably_broken;
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

    // TODO Enable when stable.
    if false {
        m = make::merge_intersections::merge_intersections(m, timer);
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
