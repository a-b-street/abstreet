//! See <https://a-b-street.github.io/docs/tech/map/importing/index.html> for an overview. This module
//! covers the RawMap->Map stage.

use std::collections::{BTreeMap, HashMap, HashSet};

use abstio::MapName;
use abstutil::{Tags, Timer};
use geom::{
    Bounds, Distance, FindClosest, GPSBounds, HashablePt2D, Line, Polygon, Speed, EPSILON_DIST,
};

pub use self::parking_lots::snap_driveway;
use crate::pathfind::Pathfinder;
use crate::raw::{OriginalRoad, RawMap};
use crate::{
    connectivity, osm, AccessRestrictions, Area, AreaID, AreaType, ControlStopSign,
    ControlTrafficSignal, Intersection, IntersectionID, IntersectionType, Lane, LaneID, Map,
    MapEdits, Movement, PathConstraints, Position, Road, RoadID, RoutingParams, Zone,
};

mod bridges;
mod buildings;
mod collapse_intersections;
pub mod initial;
mod medians;
mod merge_intersections;
mod parking_lots;
mod remove_disconnected;
pub mod traffic_signals;
mod transit;
pub mod turns;
mod walking_turns;

/// Options for converting RawMaps to Maps.
#[derive(Clone)]
pub struct RawToMapOptions {
    /// Should contraction hierarchies for pathfinding be built? They're slow to build, but without
    /// them, pathfinding on the map later will be very slow.
    pub build_ch: bool,
    /// Try to consolidate all short roads. Will likely break.
    pub consolidate_all_intersections: bool,
    /// Preserve all OSM tags for buildings, increasing the final file size substantially.
    pub keep_bldg_tags: bool,
}

impl RawToMapOptions {
    pub fn default() -> RawToMapOptions {
        RawToMapOptions {
            build_ch: true,
            consolidate_all_intersections: false,
            keep_bldg_tags: false,
        }
    }
}

impl Map {
    pub fn create_from_raw(mut raw: RawMap, opts: RawToMapOptions, timer: &mut Timer) -> Map {
        // Better to defer this and see RawMaps with more debug info in map_editor
        remove_disconnected::remove_disconnected_roads(&mut raw, timer);

        timer.start("merging short roads");
        let merged_intersections =
            merge_intersections::merge_short_roads(&mut raw, opts.consolidate_all_intersections);
        timer.stop("merging short roads");

        timer.start("collapsing degenerate intersections");
        collapse_intersections::collapse(&mut raw);
        timer.stop("collapsing degenerate intersections");

        timer.start("raw_map to InitialMap");
        let gps_bounds = raw.gps_bounds.clone();
        let bounds = gps_bounds.to_bounds();
        let initial_map = initial::InitialMap::new(&raw, &bounds, &merged_intersections, timer);
        timer.stop("raw_map to InitialMap");

        let mut map = Map {
            roads: Vec::new(),
            lanes: BTreeMap::new(),
            lane_id_counter: 0,
            intersections: Vec::new(),
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
            routing_params: RoutingParams::default(),
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
                turns: Vec::new(),
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
        for (_, r) in initial_map.roads {
            timer.next();

            let road_id = road_id_mapping[&r.id];
            let i1 = intersection_id_mapping[&r.src_i];
            let i2 = intersection_id_mapping[&r.dst_i];

            let raw_road = &raw.roads[&r.id];
            let mut road = Road {
                id: road_id,
                osm_tags: raw_road.osm_tags.clone(),
                turn_restrictions: raw_road
                    .turn_restrictions
                    .iter()
                    .filter_map(|(rt, to)| {
                        // Missing roads are filtered (like some service roads) or clipped out
                        road_id_mapping.get(to).map(|to| (*rt, *to))
                    })
                    .collect(),
                complicated_turn_restrictions: raw_road
                    .complicated_turn_restrictions
                    .iter()
                    .filter_map(|(via, to)| {
                        if let (Some(via), Some(to)) =
                            (road_id_mapping.get(via), road_id_mapping.get(to))
                        {
                            Some((*via, *to))
                        } else {
                            warn!(
                                "Complicated turn restriction from {} has invalid via {} or dst {}",
                                r.id, via, to
                            );
                            None
                        }
                    })
                    .collect(),
                orig_id: r.id,
                lanes_ltr: Vec::new(),
                center_pts: r.trimmed_center_pts,
                untrimmed_center_pts: raw_road.get_geometry(r.id, map.get_config()).unwrap().0,
                src_i: i1,
                dst_i: i2,
                speed_limit: Speed::ZERO,
                zorder: if let Some(layer) = raw_road.osm_tags.get("layer") {
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
                percent_incline: raw_road.percent_incline,
            };
            road.speed_limit = road.speed_limit_from_osm();
            road.access_restrictions = road.access_restrictions_from_osm();

            for lane in road.create_lanes(r.lane_specs_ltr, &mut map.lane_id_counter) {
                map.intersections[lane.src_i.0].outgoing_lanes.push(lane.id);
                map.intersections[lane.dst_i.0].incoming_lanes.push(lane.id);
                road.lanes_ltr.push((lane.id, lane.dir, lane.lane_type));
                map.lanes.insert(lane.id, lane);
            }

            map.roads.push(road);
        }

        for i in map.intersections.iter_mut() {
            if i.is_border() && i.roads.len() != 1 {
                panic!(
                    "{} ({}) is a border, but is connected to >1 road: {:?}",
                    i.id, i.orig_id, i.roads
                );
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
        let mut connectivity_problems = 0;
        for i in &map.intersections {
            if i.is_border() || i.is_closed() {
                continue;
            }
            if !i.is_footway(&map) && (i.incoming_lanes.is_empty() || i.outgoing_lanes.is_empty()) {
                warn!("{} is orphaned!", i.orig_id);
                continue;
            }

            let results = turns::make_all_turns(&map, i);
            if turns::verify_vehicle_connectivity(&results, i, &map).is_err() {
                connectivity_problems += 1;
            }
            all_turns.extend(results);
        }
        error!(
            "{} total intersections have some connectivity problem",
            connectivity_problems
        );
        for t in all_turns {
            assert!(map.maybe_get_t(t.id).is_none());
            if t.geom.length() < geom::EPSILON_DIST {
                warn!("{} is a very short turn", t.id);
            }
            map.intersections[t.id.parent.0].turns.push(t);
        }

        timer.start("find blackholes");
        for l in connectivity::find_scc(&map, PathConstraints::Car).1 {
            map.lanes.get_mut(&l).unwrap().driving_blackhole = true;
        }
        for l in connectivity::find_scc(&map, PathConstraints::Bike).1 {
            map.lanes.get_mut(&l).unwrap().biking_blackhole = true;
        }
        timer.stop("find blackholes");

        map.buildings =
            buildings::make_all_buildings(&raw.buildings, &map, opts.keep_bldg_tags, timer);

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
                osm_tags: Tags::empty(),
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
                        traffic_signals
                            .insert(i.id, ControlTrafficSignal::validating_new(&map, i.id));
                    }
                    Err(err) => {
                        error!(
                            "Traffic signal at {} downgraded to stop sign because of weird \
                             problem: {}",
                            i.orig_id, err
                        );
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

        if opts.build_ch {
            timer.start("setup ContractionHierarchyPathfinder");
            map.pathfinder = Pathfinder::CH(crate::pathfind::ContractionHierarchyPathfinder::new(
                &map, timer,
            ));
            timer.stop("setup ContractionHierarchyPathfinder");
        }

        map
    }
}

impl Map {
    /// Use for creating a map directly from some external format, not from a RawMap.
    pub fn import_minimal(
        name: MapName,
        bounds: Bounds,
        gps_bounds: GPSBounds,
        intersections: Vec<Intersection>,
        roads: Vec<Road>,
        lanes: Vec<Lane>,
    ) -> Map {
        let mut map = Map::blank();
        map.name = name;
        map.map_loaded_directly();
        map.bounds = bounds;
        map.gps_bounds = gps_bounds;
        map.boundary_polygon = map.bounds.get_rectangle();
        map.intersections = intersections;
        map.roads = roads;
        map.lanes = lanes.into_iter().map(|l| (l.id, l)).collect();

        let stop_signs = map
            .intersections
            .iter()
            .filter_map(|i| {
                if i.intersection_type == IntersectionType::StopSign {
                    Some(i.id)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        for i in stop_signs {
            map.stop_signs.insert(i, ControlStopSign::new(&map, i));
        }

        map
    }
}

/// Snap points to an exact Position along the nearest lane. If the result doesn't contain a
/// requested point, then there was no matching lane close enough.
pub fn match_points_to_lanes<F: Fn(&Lane) -> bool>(
    bounds: &Bounds,
    pts: HashSet<HashablePt2D>,
    lanes: &BTreeMap<LaneID, Lane>,
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
    for l in lanes.values() {
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
            pts.into_iter().collect(),
            |query_pt| {
                if let Some((l, pt)) = closest.closest_pt(query_pt.to_pt2d(), max_dist_away) {
                    if let Some(dist_along) = lanes[&l].dist_along_of_point(pt) {
                        Some((query_pt, Position::new(l, dist_along)))
                    } else {
                        panic!(
                            "{} isn't on {} according to dist_along_of_point, even though \
                             closest_point thinks it is.\n{}",
                            pt, l, lanes[&l].lane_center_pts
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

/// Adjust the path to start on the polygon's border, not center.
pub fn trim_path(poly: &Polygon, path: Line) -> Line {
    for line in poly.points().windows(2) {
        if let Some(l1) = Line::new(line[0], line[1]) {
            if let Some(hit) = l1.intersection(&path) {
                if let Some(l2) = Line::new(hit, path.pt2()) {
                    return l2;
                }
            }
        }
    }
    // Just give up
    path
}
