mod bridges;
mod buildings;
mod bus_stops;
pub mod initial;
mod remove_disconnected;
pub mod traffic_signals;
pub mod turns;

use crate::pathfind::Pathfinder;
use crate::raw::{OriginalIntersection, OriginalRoad, RawMap};
use crate::{
    connectivity, osm, Area, AreaID, BusRouteID, ControlStopSign, ControlTrafficSignal,
    Intersection, IntersectionID, IntersectionType, Lane, LaneID, Map, MapEdits, PathConstraints,
    Position, Road, RoadID, Zone,
};
use abstutil::Timer;
use enumset::EnumSet;
use geom::{Bounds, Distance, FindClosest, HashablePt2D, Polygon, Speed, EPSILON_DIST};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

impl Map {
    pub fn create_from_raw(mut raw: RawMap, build_ch: bool, timer: &mut Timer) -> Map {
        // Better to defer this and see RawMaps with more debug info in map_editor
        remove_disconnected::remove_disconnected_roads(&mut raw, timer);

        timer.start("raw_map to InitialMap");
        let gps_bounds = raw.gps_bounds.clone();
        let bounds = gps_bounds.to_bounds();
        let initial_map = initial::InitialMap::new(&raw, &bounds, timer);
        timer.stop("raw_map to InitialMap");

        let mut map = Map {
            roads: Vec::new(),
            lanes: Vec::new(),
            intersections: Vec::new(),
            turns: BTreeMap::new(),
            buildings: Vec::new(),
            bus_stops: BTreeMap::new(),
            bus_routes: Vec::new(),
            areas: Vec::new(),
            parking_lots: Vec::new(),
            zones: Vec::new(),
            boundary_polygon: raw.boundary_polygon.clone(),
            stop_signs: BTreeMap::new(),
            traffic_signals: BTreeMap::new(),
            gps_bounds,
            bounds,
            driving_side: raw.driving_side,
            pathfinder: None,
            pathfinder_dirty: false,
            city_name: raw.city_name.clone(),
            name: raw.name.clone(),
            edits: MapEdits::new(),
        };

        let road_id_mapping: BTreeMap<OriginalRoad, RoadID> = initial_map
            .roads
            .keys()
            .enumerate()
            .map(|(idx, id)| (*id, RoadID(idx)))
            .collect();
        let mut intersection_id_mapping: BTreeMap<OriginalIntersection, IntersectionID> =
            BTreeMap::new();
        for (idx, i) in initial_map.intersections.values().enumerate() {
            let id = IntersectionID(idx);
            map.intersections.push(Intersection {
                id,
                // IMPORTANT! We're relying on the triangulation algorithm not to mess with the
                // order of the points. Sidewalk corner rendering depends on it
                // later.
                polygon: Polygon::new(&i.polygon),
                turns: BTreeSet::new(),
                elevation: i.elevation,
                // Might change later
                intersection_type: i.intersection_type,
                orig_id: i.id,
                incoming_lanes: Vec::new(),
                outgoing_lanes: Vec::new(),
                roads: i.roads.iter().map(|id| road_id_mapping[id]).collect(),
            });
            intersection_id_mapping.insert(i.id, id);
        }

        timer.start_iter("expand roads to lanes", initial_map.roads.len());
        for r in initial_map.roads.values() {
            timer.next();

            let road_id = road_id_mapping[&r.id];
            let i1 = intersection_id_mapping[&r.src_i];
            let i2 = intersection_id_mapping[&r.dst_i];

            let mut road = Road {
                id: road_id,
                osm_tags: raw.roads[&r.id].osm_tags.clone(),
                turn_restrictions: raw.roads[&r.id]
                    .turn_restrictions
                    .iter()
                    .filter_map(|(rt, to)| {
                        if let Some(to) = road_id_mapping.get(to) {
                            Some((*rt, *to))
                        } else {
                            timer.warn(format!(
                                "Turn restriction from {} points to invalid dst {}",
                                r.id, to
                            ));
                            None
                        }
                    })
                    .collect(),
                complicated_turn_restrictions: raw.roads[&r.id]
                    .complicated_turn_restrictions
                    .iter()
                    .filter_map(|(via, to)| {
                        if let (Some(via), Some(to)) =
                            (road_id_mapping.get(via), road_id_mapping.get(to))
                        {
                            Some((*via, *to))
                        } else {
                            timer.warn(format!(
                                "Complicated turn restriction from {} has invalid via {} or dst {}",
                                r.id, via, to
                            ));
                            None
                        }
                    })
                    .collect(),
                orig_id: r.id,
                children_forwards: Vec::new(),
                children_backwards: Vec::new(),
                center_pts: r.trimmed_center_pts.clone(),
                src_i: i1,
                dst_i: i2,
                speed_limit: Speed::ZERO,
                zorder: if let Some(layer) = raw.roads[&r.id].osm_tags.get("layer") {
                    layer.parse::<isize>().unwrap()
                } else {
                    0
                },
                allow_through_traffic: EnumSet::new(),
            };
            road.speed_limit = road.speed_limit_from_osm();
            road.allow_through_traffic = road.access_restrictions_from_osm();

            let mut total_back_width = Distance::ZERO;
            for lane in &r.lane_specs {
                if lane.reverse_pts {
                    total_back_width += lane.width();
                }
            }
            // TODO Maybe easier to use the road's "yellow center line" and shift left/right from
            // there.
            let road_left_pts = map
                .left_shift(road.center_pts.clone(), r.half_width)
                .with_context(timer, format!("shift for {}", road.id));

            let mut fwd_width_so_far = Distance::ZERO;
            let mut back_width_so_far = Distance::ZERO;
            for lane in &r.lane_specs {
                let id = LaneID(map.lanes.len());

                let (src_i, dst_i) = if lane.reverse_pts { (i2, i1) } else { (i1, i2) };
                map.intersections[src_i.0].outgoing_lanes.push(id);
                map.intersections[dst_i.0].incoming_lanes.push(id);

                road.children_mut(!lane.reverse_pts)
                    .push((id, lane.lane_type));

                // Careful about order here. lane_specs are all of the forwards from center to
                // sidewalk, then all the backwards from center to sidewalk.
                let lane_center_pts = if !lane.reverse_pts {
                    let pl = map.right_shift(
                        road_left_pts.clone(),
                        total_back_width + fwd_width_so_far + (lane.width() / 2.0),
                    );
                    fwd_width_so_far += lane.width();
                    pl.with_context(timer, format!("shift for {}", id))
                } else {
                    let pl = map.right_shift(
                        road_left_pts.clone(),
                        total_back_width - back_width_so_far - (lane.width() / 2.0),
                    );
                    back_width_so_far += lane.width();
                    pl.with_context(timer, format!("shift for {}", id))
                        .reversed()
                };

                map.lanes.push(Lane {
                    id,
                    lane_center_pts,
                    width: lane.width(),
                    src_i,
                    dst_i,
                    lane_type: lane.lane_type,
                    parent: road_id,
                    building_paths: Vec::new(),
                    bus_stops: BTreeSet::new(),
                    parking_blackhole: None,
                });
            }
            if road.get_name() == "???" {
                timer.warn(format!(
                    "{} has no name. Tags: {:?}",
                    road.id, road.osm_tags
                ));
            }
            map.roads.push(road);
        }

        for i in map.intersections.iter_mut() {
            if is_border(i, &map.lanes) {
                i.intersection_type = IntersectionType::Border;
            }
            if i.is_border() {
                if i.roads.len() != 1 {
                    panic!(
                        "{} ({}) is a border, but is connected to >1 road: {:?}",
                        i.id, i.orig_id, i.roads
                    );
                }
                continue;
            }
            if i.is_closed() {
                continue;
            }
            if i.intersection_type == IntersectionType::TrafficSignal {
                let mut ok = false;
                for r in &i.roads {
                    if map.roads[r.0].osm_tags.get(osm::HIGHWAY)
                        != Some(&"construction".to_string())
                    {
                        ok = true;
                        break;
                    }
                }
                if !ok {
                    i.intersection_type = IntersectionType::StopSign;
                }
            }

            if i.incoming_lanes.is_empty() || i.outgoing_lanes.is_empty() {
                timer.warn(format!("{} is orphaned!", i.orig_id));
                continue;
            }

            for t in turns::make_all_turns(map.driving_side, i, &map.roads, &map.lanes, timer) {
                assert!(!map.turns.contains_key(&t.id));
                i.turns.insert(t.id);
                if t.geom.length() < geom::EPSILON_DIST {
                    timer.warn(format!("{} is a very short turn", t.id));
                }
                map.turns.insert(t.id, t);
            }
        }

        timer.start("find parking blackholes");
        for (l, redirect) in connectivity::redirect_parking_blackholes(&map, timer) {
            map.lanes[l.0].parking_blackhole = Some(redirect);
        }
        timer.stop("find parking blackholes");

        map.buildings = buildings::make_all_buildings(&raw.buildings, &map, timer);
        for b in &map.buildings {
            let lane = b.sidewalk();

            // TODO Could be more performant and cleanly written
            let mut bldgs = map.lanes[lane.0].building_paths.clone();
            bldgs.push(b.id);
            bldgs.sort_by_key(|b| map.buildings[b.0].front_path.sidewalk.dist_along());
            map.lanes[lane.0].building_paths = bldgs;
        }

        map.parking_lots =
            buildings::make_all_parking_lots(&raw.parking_lots, &raw.parking_aisles, &map, timer);

        map.zones = Zone::make_all(&map);

        for (idx, a) in raw.areas.iter().enumerate() {
            map.areas.push(Area {
                id: AreaID(idx),
                area_type: a.area_type,
                polygon: a.polygon.clone(),
                osm_tags: a.osm_tags.clone(),
                osm_id: a.osm_id,
            });
        }

        bridges::find_bridges(&mut map.roads, &map.bounds, timer);

        let mut stop_signs: BTreeMap<IntersectionID, ControlStopSign> = BTreeMap::new();
        let mut traffic_signals: BTreeMap<IntersectionID, ControlTrafficSignal> = BTreeMap::new();
        for i in &map.intersections {
            match i.intersection_type {
                IntersectionType::StopSign => {
                    stop_signs.insert(i.id, ControlStopSign::new(&map, i.id));
                }
                IntersectionType::TrafficSignal => {
                    traffic_signals.insert(i.id, ControlTrafficSignal::new(&map, i.id, timer));
                }
                IntersectionType::Border | IntersectionType::Construction => {}
            };
        }
        map.stop_signs = stop_signs;
        map.traffic_signals = traffic_signals;

        traffic_signals::synchronize(&mut map);

        // Here's a fun one: we can't set up walking_using_transit yet, because we haven't
        // finalized bus stops and routes. We need the bus graph in place for that. So setup
        // pathfinding in two stages.
        if build_ch {
            timer.start("setup (most of) Pathfinder");
            map.pathfinder = Some(Pathfinder::new_without_transit(&map, timer));
            timer.stop("setup (most of) Pathfinder");

            {
                // Turn the two directions of each route into one loop. Need to do something better
                // with borders later.
                for r in &mut raw.bus_routes {
                    r.fwd_stops.extend(r.back_stops.drain(..));
                }

                let (stops, routes) = bus_stops::make_bus_stops(&mut map, &raw.bus_routes, timer);
                map.bus_stops = stops;

                timer.start_iter("verify bus routes are connected", routes.len());
                for mut r in routes {
                    timer.next();
                    if r.stops.is_empty() {
                        continue;
                    }
                    if bus_stops::fix_bus_route(&map, &mut r) {
                        r.id = BusRouteID(map.bus_routes.len());
                        map.bus_routes.push(r);
                    } else {
                        timer.warn(format!("Skipping route {}", r.name));
                    }
                }

                // Remove orphaned bus stops. This messes up the BusStopID indexing.
                for id in map
                    .bus_stops
                    .keys()
                    .filter(|id| map.get_routes_serving_stop(**id).is_empty())
                    .cloned()
                    .collect::<Vec<_>>()
                {
                    map.bus_stops.remove(&id);
                    map.lanes[id.sidewalk.0].bus_stops.remove(&id);
                }
            }

            timer.start("setup rest of Pathfinder (walking with transit)");
            let mut pathfinder = map.pathfinder.take().unwrap();
            pathfinder.setup_walking_with_transit(&map);
            map.pathfinder = Some(pathfinder);
            timer.stop("setup rest of Pathfinder (walking with transit)");
        }

        let (_, disconnected) = connectivity::find_scc(&map, PathConstraints::Pedestrian);
        if !disconnected.is_empty() {
            timer.warn(format!(
                "{} sidewalks are disconnected!",
                disconnected.len()
            ));
            for l in disconnected {
                // Best response is to use map_editor to delete them. Hard to do automatically
                // because maybe there are bus stops nearby -- force myself to look at it manually.
                timer.warn(format!("- Sidewalk {} is disconnected", l));
            }
        }

        map
    }
}

fn is_border(intersection: &Intersection, lanes: &Vec<Lane>) -> bool {
    // RawIntersection said it is.
    if intersection.is_border() {
        return true;
    }

    // This only detects one-way borders! Two-way ones will just look like dead-ends.

    // Bias for driving
    if intersection.roads.len() != 1 {
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

// If the result doesn't contain a requested point, then there was no matching lane close
// enough.
fn match_points_to_lanes<F: Fn(&Lane) -> bool>(
    bounds: &Bounds,
    pts: HashSet<HashablePt2D>,
    lanes: &Vec<Lane>,
    filter: F,
    buffer: Distance,
    max_dist_away: Distance,
    timer: &mut Timer,
) -> HashMap<HashablePt2D, Position> {
    if pts.is_empty() {
        return HashMap::new();
    }

    let mut closest: FindClosest<LaneID> = FindClosest::new(bounds);
    timer.start_iter("index lanes", lanes.len());
    for l in lanes {
        timer.next();
        if filter(l) && l.length() > (buffer + EPSILON_DIST) * 2.0 {
            closest.add(
                l.id,
                l.lane_center_pts
                    .exact_slice(buffer, l.lane_center_pts.length() - buffer)
                    .points(),
            );
        }
    }

    // For each point, find the closest point to any lane, using the quadtree to prune the
    // search.
    let mut results: HashMap<HashablePt2D, Position> = HashMap::new();
    timer.start_iter("find closest lane point", pts.len());
    for query_pt in pts {
        timer.next();
        if let Some((l, pt)) = closest.closest_pt(query_pt.to_pt2d(), max_dist_away) {
            if let Some(dist_along) = lanes[l.0].dist_along_of_point(pt) {
                results.insert(query_pt, Position::new(l, dist_along));
            } else {
                panic!(
                    "{} isn't on {} according to dist_along_of_point, even though closest_point \
                     thinks it is.\n{}",
                    pt, l, lanes[l.0].lane_center_pts
                );
            }
        }
    }
    results
}
