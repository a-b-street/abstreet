//! See <https://dabreegster.github.io/abstreet/map/importing/index.html> for an overview. This module
//! covers the RawMap->Map stage.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use abstutil::{Parallelism, Tags, Timer};
use geom::{Bounds, Distance, FindClosest, HashablePt2D, Speed, EPSILON_DIST};

use crate::pathfind::Pathfinder;
use crate::raw::{OriginalRoad, RawMap};
use crate::{
    connectivity, osm, AccessRestrictions, Area, AreaID, AreaType, ControlStopSign,
    ControlTrafficSignal, Direction, Intersection, IntersectionID, IntersectionType, Lane, LaneID,
    Map, MapEdits, Movement, PathConstraints, Position, Road, RoadID, Zone,
};

mod bridges;
mod buildings;
pub mod initial;
mod medians;
mod merge_intersections;
mod parking_lots;
mod remove_disconnected;
pub mod traffic_signals;
mod transit;
pub mod turns;
mod walking_turns;

impl Map {
    pub fn create_from_raw(
        mut raw: RawMap,
        build_ch: bool,
        keep_bldg_tags: bool,
        timer: &mut Timer,
    ) -> Map {
        // Better to defer this and see RawMaps with more debug info in map_editor
        remove_disconnected::remove_disconnected_roads(&mut raw, timer);

        let merged_intersections = merge_intersections::merge_short_roads(&mut raw);

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
            config: raw.config.clone(),
            pathfinder: Pathfinder::Dijkstra,
            pathfinder_dirty: false,
            name: raw.name.clone(),
            edits: MapEdits::new(),
        };
        map.edits = map.new_edits();

        let road_id_mapping: BTreeMap<OriginalRoad, RoadID> = initial_map
            .roads
            .keys()
            .enumerate()
            .map(|(idx, id)| (*id, RoadID(idx)))
            .collect();
        let mut intersection_id_mapping: BTreeMap<osm::NodeID, IntersectionID> = BTreeMap::new();
        for (idx, i) in initial_map.intersections.values().enumerate() {
            let id = IntersectionID(idx);
            map.intersections.push(Intersection {
                id,
                polygon: i.polygon.clone(),
                turns: BTreeSet::new(),
                elevation: i.elevation,
                // Might change later
                intersection_type: i.intersection_type,
                orig_id: i.id,
                incoming_lanes: Vec::new(),
                outgoing_lanes: Vec::new(),
                roads: i.roads.iter().map(|id| road_id_mapping[id]).collect(),
                merged: merged_intersections.contains(&i.id),
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
                        // Missing roads are filtered (like service roads) or clipped out
                        road_id_mapping.get(to).map(|to| (*rt, *to))
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
                lanes_ltr: Vec::new(),
                center_pts: r.trimmed_center_pts.clone(),
                src_i: i1,
                dst_i: i2,
                speed_limit: Speed::ZERO,
                zorder: if let Some(layer) = raw.roads[&r.id].osm_tags.get("layer") {
                    match layer.parse::<f64>() {
                        // Just drop .5 for now
                        Ok(l) => l as isize,
                        Err(_) => {
                            warn!("Weird layer={} on {}", layer, r.id);
                            0
                        }
                    }
                } else {
                    0
                },
                access_restrictions: AccessRestrictions::new(),
            };
            road.speed_limit = road.speed_limit_from_osm();
            road.access_restrictions = road.access_restrictions_from_osm();

            let mut total_back_width = Distance::ZERO;
            for lane in &r.lane_specs_ltr {
                if lane.dir == Direction::Back {
                    total_back_width += lane.width;
                }
            }
            // TODO Maybe easier to use the road's "yellow center line" and shift left/right from
            // there.
            let road_left_pts = road
                .center_pts
                .shift_left(r.half_width)
                .unwrap_or_else(|_| road.center_pts.clone());

            let mut width_so_far = Distance::ZERO;
            for lane in &r.lane_specs_ltr {
                let id = LaneID(map.lanes.len());

                let (src_i, dst_i) = if lane.dir == Direction::Fwd {
                    (i1, i2)
                } else {
                    (i2, i1)
                };
                map.intersections[src_i.0].outgoing_lanes.push(id);
                map.intersections[dst_i.0].incoming_lanes.push(id);

                road.lanes_ltr.push((id, lane.dir, lane.lt));

                let pl =
                    if let Ok(pl) = road_left_pts.shift_right(width_so_far + (lane.width / 2.0)) {
                        pl
                    } else {
                        timer.error(format!("{} geometry broken; lane not shifted!", id));
                        road_left_pts.clone()
                    };
                let lane_center_pts = if lane.dir == Direction::Fwd {
                    pl
                } else {
                    pl.reversed()
                };
                width_so_far += lane.width;

                map.lanes.push(Lane {
                    id,
                    lane_center_pts,
                    width: lane.width,
                    src_i,
                    dst_i,
                    lane_type: lane.lt,
                    parent: road_id,
                    bus_stops: BTreeSet::new(),
                    driving_blackhole: false,
                    biking_blackhole: false,
                });
            }
            map.roads.push(road);
        }

        for i in map.intersections.iter_mut() {
            if i.is_border() {
                if i.roads.len() != 1 {
                    panic!(
                        "{} ({}) is a border, but is connected to >1 road: {:?}",
                        i.id, i.orig_id, i.roads
                    );
                }
            }
            if i.intersection_type == IntersectionType::TrafficSignal {
                let mut ok = false;
                for r in &i.roads {
                    // Skip signals only connected to roads under construction or purely to control
                    // light rail tracks.
                    if !map.roads[r.0].osm_tags.is(osm::HIGHWAY, "construction")
                        && !map.roads[r.0].is_light_rail()
                    {
                        ok = true;
                        break;
                    }
                }
                if !ok {
                    i.intersection_type = IntersectionType::StopSign;
                }
            }
        }

        let mut all_turns = Vec::new();
        for i in &map.intersections {
            if i.is_border() || i.is_closed() {
                continue;
            }
            if !i.is_footway(&map) && (i.incoming_lanes.is_empty() || i.outgoing_lanes.is_empty()) {
                timer.warn(format!("{} is orphaned!", i.orig_id));
                continue;
            }

            all_turns.extend(turns::make_all_turns(&map, i, timer));
        }
        for t in all_turns {
            assert!(!map.turns.contains_key(&t.id));
            map.intersections[t.id.parent.0].turns.insert(t.id);
            if t.geom.length() < geom::EPSILON_DIST {
                timer.warn(format!("{} is a very short turn", t.id));
            }
            map.turns.insert(t.id, t);
        }

        timer.start("find blackholes");
        for l in connectivity::find_scc(&map, PathConstraints::Car).1 {
            map.lanes[l.0].driving_blackhole = true;
        }
        for l in connectivity::find_scc(&map, PathConstraints::Bike).1 {
            map.lanes[l.0].biking_blackhole = true;
        }
        timer.stop("find blackholes");

        map.buildings = buildings::make_all_buildings(&raw.buildings, &map, keep_bldg_tags, timer);

        map.parking_lots = parking_lots::make_all_parking_lots(
            &raw.parking_lots,
            &raw.parking_aisles,
            &map,
            timer,
        );

        map.zones = Zone::make_all(&map);

        // Create medians first, so they wind up rendering underneath areas from OSM. Sometimes
        // medians contain mapped grass.
        for polygon in medians::find_medians(&map) {
            map.areas.push(Area {
                id: AreaID(map.areas.len()),
                area_type: AreaType::MedianStrip,
                polygon,
                osm_tags: Tags::new(BTreeMap::new()),
                osm_id: None,
            });
        }
        for a in &raw.areas {
            map.areas.push(Area {
                id: AreaID(map.areas.len()),
                area_type: a.area_type,
                polygon: a.polygon.clone(),
                osm_tags: a.osm_tags.clone(),
                osm_id: Some(a.osm_id),
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
                IntersectionType::TrafficSignal => match Movement::for_i(i.id, &map) {
                    Ok(_) => {
                        traffic_signals.insert(i.id, ControlTrafficSignal::new(&map, i.id, timer));
                    }
                    Err(err) => {
                        timer.error(format!(
                            "Traffic signal at {} downgraded to stop sign because of weird \
                             problem: {}",
                            i.orig_id, err
                        ));
                        stop_signs.insert(i.id, ControlStopSign::new(&map, i.id));
                    }
                },
                IntersectionType::Border | IntersectionType::Construction => {}
            };
        }
        map.stop_signs = stop_signs;
        map.traffic_signals = traffic_signals;
        // Fix up the type for any problematic traffic signals
        for i in map.stop_signs.keys() {
            map.intersections[i.0].intersection_type = IntersectionType::StopSign;
        }

        traffic_signals::synchronize(&mut map);

        // Note this will always use the slower Pathfinder::Dijkstra.
        transit::make_stops_and_routes(&mut map, &raw.bus_routes, timer);
        for id in map.bus_stops.keys() {
            assert!(!map.get_routes_serving_stop(*id).is_empty());
        }

        if build_ch {
            timer.start("setup ContractionHierarchyPathfinder");
            map.pathfinder = Pathfinder::CH(crate::pathfind::ContractionHierarchyPathfinder::new(
                &map, timer,
            ));
            timer.stop("setup ContractionHierarchyPathfinder");
        }

        map
    }
}

/// Snap points to an exact Position along the nearest lane. If the result doesn't contain a
/// requested point, then there was no matching lane close enough.
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
    timer
        .parallelize(
            "find closest lane point",
            Parallelism::Fastest,
            pts.into_iter().collect(),
            |query_pt| {
                if let Some((l, pt)) = closest.closest_pt(query_pt.to_pt2d(), max_dist_away) {
                    if let Some(dist_along) = lanes[l.0].dist_along_of_point(pt) {
                        Some((query_pt, Position::new(l, dist_along)))
                    } else {
                        panic!(
                            "{} isn't on {} according to dist_along_of_point, even though \
                             closest_point thinks it is.\n{}",
                            pt, l, lanes[l.0].lane_center_pts
                        );
                    }
                } else {
                    None
                }
            },
        )
        .into_iter()
        .flatten()
        .collect()
}
