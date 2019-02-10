use crate::{
    make, raw_data, Area, AreaID, Building, Intersection, IntersectionID, IntersectionType, Lane,
    LaneID, Parcel, Road, RoadID, Turn, TurnID, LANE_THICKNESS,
};
use abstutil::Timer;
use geom::{Bounds, GPSBounds, Polygon, Pt2D};
use std::collections::BTreeMap;

pub struct HalfMap {
    pub roads: Vec<Road>,
    pub lanes: Vec<Lane>,
    pub intersections: Vec<Intersection>,
    pub turns: BTreeMap<TurnID, Turn>,
    pub buildings: Vec<Building>,
    pub parcels: Vec<Parcel>,
    pub areas: Vec<Area>,

    pub turn_lookup: Vec<TurnID>,
}

pub fn make_half_map(
    data: &raw_data::Map,
    initial_map: make::InitialMap,
    gps_bounds: &GPSBounds,
    bounds: &Bounds,
    timer: &mut Timer,
) -> HalfMap {
    let mut half_map = HalfMap {
        roads: Vec::new(),
        lanes: Vec::new(),
        intersections: Vec::new(),
        turns: BTreeMap::new(),
        buildings: Vec::new(),
        parcels: Vec::new(),
        areas: Vec::new(),
        turn_lookup: Vec::new(),
    };

    let road_id_mapping: BTreeMap<raw_data::StableRoadID, RoadID> = initial_map
        .roads
        .keys()
        .enumerate()
        .map(|(idx, id)| (*id, RoadID(idx)))
        .collect();
    let mut intersection_id_mapping: BTreeMap<raw_data::StableIntersectionID, IntersectionID> =
        BTreeMap::new();
    for (idx, i) in initial_map.intersections.values().enumerate() {
        let raw_i = &data.intersections[&i.id];

        let id = IntersectionID(idx);
        let pt = Pt2D::from_gps(raw_i.point, &gps_bounds).unwrap();
        half_map.intersections.push(Intersection {
            id,
            point: pt,
            // IMPORTANT! We're relying on the triangulation algorithm not to mess with the order
            // of the points. Sidewalk corner rendering depends on it later.
            polygon: Polygon::new(&i.polygon),
            turns: Vec::new(),
            elevation: raw_i.elevation,
            // Might change later
            intersection_type: raw_i.intersection_type,
            label: raw_i.label.clone(),
            stable_id: i.id,
            incoming_lanes: Vec::new(),
            outgoing_lanes: Vec::new(),
            roads: i.roads.iter().map(|id| road_id_mapping[id]).collect(),
        });
        intersection_id_mapping.insert(i.id, id);
    }

    timer.start_iter("expand roads to lanes", initial_map.roads.len());
    for r in initial_map.roads.values() {
        timer.next();

        let raw_r = &data.roads[&r.id];
        let road_id = road_id_mapping[&r.id];
        let i1 = intersection_id_mapping[&r.src_i];
        let i2 = intersection_id_mapping[&r.dst_i];

        let mut road = Road {
            id: road_id,
            osm_tags: raw_r.osm_tags.clone(),
            osm_way_id: raw_r.osm_way_id,
            stable_id: r.id,
            children_forwards: Vec::new(),
            children_backwards: Vec::new(),
            center_pts: r.trimmed_center_pts.clone(),
            original_center_pts: r.original_center_pts.clone(),
            src_i: i1,
            dst_i: i2,
        };

        for lane in &r.lane_specs {
            let id = LaneID(half_map.lanes.len());

            let (src_i, dst_i) = if lane.reverse_pts { (i2, i1) } else { (i1, i2) };
            half_map.intersections[src_i.0].outgoing_lanes.push(id);
            half_map.intersections[dst_i.0].incoming_lanes.push(id);

            let (unshifted_pts, offset) = if lane.reverse_pts {
                road.children_backwards.push((id, lane.lane_type));
                (
                    road.center_pts.reversed(),
                    road.children_backwards.len() - 1,
                )
            } else {
                road.children_forwards.push((id, lane.lane_type));
                (road.center_pts.clone(), road.children_forwards.len() - 1)
            };
            // TODO probably different behavior for oneways
            // TODO need to factor in yellow center lines (but what's the right thing to even do?
            // Reverse points for British-style driving on the left
            let width = LANE_THICKNESS * (0.5 + (offset as f64));
            let lane_center_pts = unshifted_pts.shift_right(width);

            half_map.lanes.push(Lane {
                id,
                lane_center_pts,
                src_i,
                dst_i,
                lane_type: lane.lane_type,
                parent: road_id,
                building_paths: Vec::new(),
                bus_stops: Vec::new(),
            });
        }
        half_map.roads.push(road);
    }

    for i in half_map.intersections.iter_mut() {
        if i.incoming_lanes.is_empty() && i.outgoing_lanes.is_empty() {
            panic!("{:?} is orphaned!", i);
        }

        // Is the intersection a border?
        if is_border(i, &half_map.lanes) {
            i.intersection_type = IntersectionType::Border;
        }

        for t in make::turns::make_all_turns(i, &half_map.roads, &half_map.lanes) {
            assert!(!half_map.turns.contains_key(&t.id));
            i.turns.push(t.id);
            half_map.turns.insert(t.id, t);
        }
    }

    for t in half_map.turns.values_mut() {
        t.lookup_idx = half_map.turn_lookup.len();
        half_map.turn_lookup.push(t.id);
        if t.geom.length() < geom::EPSILON_DIST {
            warn!("u{} is a very short turn", t.lookup_idx);
        }
    }

    make::make_all_buildings(
        &mut half_map.buildings,
        &data.buildings,
        &gps_bounds,
        &bounds,
        &half_map.lanes,
        timer,
    );
    for b in &half_map.buildings {
        let lane = b.sidewalk();

        // TODO Could be more performant and cleanly written
        let mut bldgs = half_map.lanes[lane.0].building_paths.clone();
        bldgs.push(b.id);
        bldgs.sort_by_key(|b| half_map.buildings[b.0].front_path.sidewalk.dist_along());
        half_map.lanes[lane.0].building_paths = bldgs;
    }

    make::make_all_parcels(
        &mut half_map.parcels,
        &data.parcels,
        &gps_bounds,
        &bounds,
        &half_map.lanes,
        timer,
    );

    for (idx, a) in data.areas.iter().enumerate() {
        half_map.areas.push(Area {
            id: AreaID(idx),
            area_type: a.area_type,
            points: a
                .points
                .iter()
                .map(|coord| Pt2D::from_gps(*coord, &gps_bounds).unwrap())
                .collect(),
            osm_tags: a.osm_tags.clone(),
            osm_id: a.osm_id,
        });
    }

    half_map
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
