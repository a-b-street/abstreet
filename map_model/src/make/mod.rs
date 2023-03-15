//! See <https://a-b-street.github.io/docs/tech/map/importing/index.html> for an overview. This module
//! covers the RawMap->Map stage.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use structopt::StructOpt;

use abstutil::{MultiMap, Tags, Timer};
use geom::{
    Distance, FindClosest, HashablePt2D, Line, PolyLine, Polygon, Pt2D, Speed, EPSILON_DIST,
};
use osm2streets::Transformation;
use raw_map::RawMap;

pub use self::parking_lots::snap_driveway;
use crate::pathfind::{CreateEngine, Pathfinder};
use crate::{
    connectivity, osm, AccessRestrictions, Area, AreaID, ControlStopSign, ControlTrafficSignal,
    Intersection, IntersectionControl, IntersectionID, IntersectionKind, Lane, LaneID, Map,
    MapEdits, OriginalRoad, PathConstraints, Position, Road, RoadID, RoutingParams, Zone,
};

mod bridges;
mod buildings;
mod parking_lots;
pub mod traffic_signals;
pub mod transit;
pub mod turns;
mod walking_turns;

/// Options for converting RawMaps to Maps.
#[derive(Clone, Default, StructOpt)]
pub struct RawToMapOptions {
    /// Should contraction hierarchies for pathfinding be built? They're slow to build, but without
    /// them, pathfinding on the map later will be very slow.
    #[structopt(long)]
    pub skip_ch: bool,
    /// Preserve all OSM tags for buildings, increasing the final file size substantially.
    #[structopt(long)]
    pub keep_bldg_tags: bool,
}

impl Map {
    pub fn create_from_raw(mut raw: RawMap, opts: RawToMapOptions, timer: &mut Timer) -> Map {
        raw.streets
            .apply_transformations(Transformation::abstreet(), timer);

        let mut map = Map {
            roads: Vec::new(),
            intersections: Vec::new(),
            buildings: Vec::new(),
            transit_stops: BTreeMap::new(),
            transit_routes: Vec::new(),
            areas: Vec::new(),
            parking_lots: Vec::new(),
            zones: Vec::new(),
            census_zones: raw.census_zones.clone(),
            extra_pois: raw.extra_pois.clone(),
            boundary_polygon: raw.streets.boundary_polygon.clone(),
            stop_signs: BTreeMap::new(),
            traffic_signals: BTreeMap::new(),
            bus_routes_on_roads: std::mem::take(&mut raw.bus_routes_on_roads),
            gps_bounds: raw.streets.gps_bounds.clone(),
            bounds: raw.streets.gps_bounds.to_bounds(),
            config: raw.streets.config.clone(),
            pathfinder: Pathfinder::empty(),
            pathfinder_dirty: false,
            routing_params: RoutingParams::default(),
            name: raw.name.clone(),
            edits: MapEdits::new(),
            edits_generation: 0,
            road_to_buildings: MultiMap::new(),
        };
        map.edits = map.new_edits();

        let road_id_mapping: BTreeMap<osm2streets::RoadID, RoadID> = raw
            .streets
            .roads
            .keys()
            .enumerate()
            .map(|(idx, id)| (*id, RoadID(idx)))
            .collect();
        let mut intersection_id_mapping: BTreeMap<osm2streets::IntersectionID, IntersectionID> =
            BTreeMap::new();
        for (idx, i) in raw.streets.intersections.values().enumerate() {
            let id = IntersectionID(idx);
            map.intersections.push(Intersection {
                id,
                polygon: i.polygon.clone(),
                turns: Vec::new(),
                movements: BTreeMap::new(),
                elevation: raw.elevation_per_intersection[&i.id],
                // Might change later
                kind: i.kind,
                control: match i.control {
                    // Nothing in A/B Street handles uncontrolled intersections yet
                    IntersectionControl::Uncontrolled => IntersectionControl::Signed,
                    x => x,
                },
                // TODO Hack. Handle multiple OSM IDs everywhere instead
                orig_id: i
                    .osm_ids
                    .get(0)
                    .cloned()
                    .unwrap_or(osm::NodeID(-1 * (map.intersections.len() as i64))),
                incoming_lanes: Vec::new(),
                outgoing_lanes: Vec::new(),
                roads: i.roads.iter().map(|id| road_id_mapping[id]).collect(),
                merged: !raw.streets.intersections[&i.id]
                    .trim_roads_for_merging
                    .is_empty(),
            });
            intersection_id_mapping.insert(i.id, id);
        }

        timer.start_iter("expand roads to lanes", raw.streets.roads.len());
        for r in raw.streets.roads.values_mut() {
            timer.next();

            let road_id = road_id_mapping[&r.id];
            let i1 = intersection_id_mapping[&r.src_i];
            let i2 = intersection_id_mapping[&r.dst_i];

            let extra = &raw.extra_road_data[&r.id];
            let barrier_nodes = snap_nodes_to_line(&extra.barrier_nodes, &r.center_line);
            let crossing_nodes =
                snap_nodes_with_data_to_line(&extra.crossing_nodes, &r.center_line);

            // TODO Hack. Roads and intersections each may have ZERO or more OSM IDs.
            let orig_id = OriginalRoad {
                osm_way_id: r
                    .osm_ids
                    .get(0)
                    .cloned()
                    .unwrap_or(osm::WayID(-1 * (map.roads.len() as i64))),
                i1: map.intersections[i1.0].orig_id,
                i2: map.intersections[i2.0].orig_id,
            };

            let mut road = Road {
                id: road_id,
                // Arbitrarily remember OSM tags from one of the ways
                // TODO If this road was introduced synthetically, we'll have empty tags, which
                // might break various downstream bits of code
                osm_tags: if let Some(id) = r.osm_ids.get(0) {
                    raw.osm_tags[id].clone()
                } else {
                    Tags::empty()
                },
                orig_id,
                turn_restrictions: r
                    .turn_restrictions
                    .iter()
                    .filter_map(|(rt, to)| {
                        // Missing roads are filtered (like some service roads) or clipped out
                        road_id_mapping.get(to).map(|to| (*rt, *to))
                    })
                    .collect(),
                complicated_turn_restrictions: r
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
                lanes: Vec::new(),
                center_pts: r.center_line.clone(),
                untrimmed_center_pts: r.get_untrimmed_center_line(raw.streets.config.driving_side),
                trim_start: r.trim_start,
                trim_end: r.trim_end,
                src_i: i1,
                dst_i: i2,
                speed_limit: Speed::ZERO,
                zorder: r.layer,
                access_restrictions: AccessRestrictions::new(),
                percent_incline: extra.percent_incline,
                crosswalk_forward: extra.crosswalk_forward,
                crosswalk_backward: extra.crosswalk_backward,
                transit_stops: BTreeSet::new(),
                barrier_nodes,
                crossing_nodes,
            };
            road.speed_limit = road.speed_limit_from_osm();
            road.access_restrictions = road.access_restrictions_from_osm();

            road.recreate_lanes(r.lane_specs_ltr.clone());
            for lane in &road.lanes {
                map.intersections[lane.src_i.0].outgoing_lanes.push(lane.id);
                map.intersections[lane.dst_i.0].incoming_lanes.push(lane.id);
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
            if i.control == IntersectionControl::Signalled {
                let mut ok = true;
                for r in &i.roads {
                    let road = &map.roads[r.0];
                    // Skip signals only connected to roads under construction or purely to control
                    // light rail tracks.
                    if road.osm_tags.is(osm::HIGHWAY, "construction") || road.is_light_rail() {
                        ok = false;
                        break;
                    }
                    // Skip signals that likely don't have correct intersection geometry
                    if road.trim_start == Distance::ZERO || road.trim_end == Distance::ZERO {
                        ok = false;
                        break;
                    }
                }
                if !ok {
                    i.control = IntersectionControl::Signed;
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
            map.mut_lane(l).driving_blackhole = true;
        }
        for l in connectivity::find_scc(&map, PathConstraints::Bike).1 {
            map.mut_lane(l).biking_blackhole = true;
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

        for a in &raw.areas {
            map.areas.push(Area {
                id: AreaID(map.areas.len()),
                area_type: a.area_type,
                polygon: a.polygon.clone(),
                osm_tags: a.osm_tags.clone(),
                osm_id: Some(a.osm_id),
            });
        }

        bridges::find_bridges(&mut map.roads, timer);

        map.recalculate_all_movements(timer);

        let mut stop_signs: BTreeMap<IntersectionID, ControlStopSign> = BTreeMap::new();
        let mut traffic_signals: BTreeMap<IntersectionID, ControlTrafficSignal> = BTreeMap::new();
        for i in &map.intersections {
            if i.kind == IntersectionKind::MapEdge {
                continue;
            }
            match i.control {
                IntersectionControl::Signed | IntersectionControl::Uncontrolled => {
                    stop_signs.insert(i.id, ControlStopSign::new(&map, i.id));
                }
                IntersectionControl::Signalled => {
                    if i.movements.is_empty() {
                        error!("Traffic signal at {} downgraded to stop sign, because it has no movements -- probably roads under construction", i.orig_id);
                        stop_signs.insert(i.id, ControlStopSign::new(&map, i.id));
                    } else {
                        traffic_signals.insert(
                            i.id,
                            ControlTrafficSignal::get_possible_policies(&map, i.id)
                                .remove(0)
                                .1,
                        );
                    }
                }
                IntersectionControl::Construction => {}
            };
        }
        map.stop_signs = stop_signs;
        map.traffic_signals = traffic_signals;
        // Fix up the type for any problematic traffic signals
        for i in map.stop_signs.keys() {
            map.intersections[i.0].control = IntersectionControl::Signed;
        }

        traffic_signals::synchronize(&mut map);

        timer.start("setup pathfinding");
        let engine = if opts.skip_ch {
            CreateEngine::Dijkstra
        } else {
            CreateEngine::CH
        };
        map.pathfinder = Pathfinder::new(&map, map.routing_params().clone(), &engine, timer);
        timer.stop("setup pathfinding");

        transit::finalize_transit(&mut map, &raw, timer);
        timer.start("setup pathfinding for people using transit");
        let mut pathfinder = std::mem::replace(&mut map.pathfinder, Pathfinder::empty());
        pathfinder.finalize_transit(&map, &engine);
        map.pathfinder = pathfinder;
        timer.stop("setup pathfinding for people using transit");

        map
    }
}

/// Snap points to an exact Position along the nearest lane. If the result doesn't contain a
/// requested point, then there was no matching lane close enough.
pub fn match_points_to_lanes<F: Fn(&Lane) -> bool>(
    map: &Map,
    pts: HashSet<HashablePt2D>,
    filter: F,
    buffer: Distance,
    max_dist_away: Distance,
    timer: &mut Timer,
) -> HashMap<HashablePt2D, Position> {
    if pts.is_empty() {
        return HashMap::new();
    }

    let mut closest: FindClosest<LaneID> = FindClosest::new();
    timer.start_iter("index lanes", map.all_lanes().count());
    for l in map.all_lanes() {
        timer.next();
        if filter(l) && l.length() > (buffer + EPSILON_DIST) * 2.0 {
            closest.add(
                l.id,
                l.lane_center_pts
                    .exact_slice(buffer, l.length() - buffer)
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
                    if let Some(dist_along) = map.get_l(l).dist_along_of_point(pt) {
                        Some((query_pt, Position::new(l, dist_along)))
                    } else {
                        panic!(
                            "{} isn't on {} according to dist_along_of_point, even though \
                             closest_point thinks it is.\n{}",
                            pt,
                            l,
                            map.get_l(l).lane_center_pts
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
    for line in poly.get_outer_ring().points().windows(2) {
        if let Ok(l1) = Line::new(line[0], line[1]) {
            if let Some(hit) = l1.intersection(&path) {
                if let Ok(l2) = Line::new(hit, path.pt2()) {
                    return l2;
                }
            }
        }
    }
    // Just give up
    path
}

// TODO Duplicate code with below. One caller needs data, the other doesn't and adding fake data is
// annoying
fn snap_nodes_to_line(pts: &[Pt2D], pl: &PolyLine) -> Vec<Distance> {
    let mut results = Vec::new();
    for pt in pts {
        let projected = pl.project_pt(*pt);
        // TODO Check distance isn't too crazy? Not sure why it would be
        if let Some((dist, _)) = pl.dist_along_of_point(projected) {
            results.push(dist);
        }
    }
    results
}

fn snap_nodes_with_data_to_line<T: Clone>(
    input: &[(Pt2D, T)],
    pl: &PolyLine,
) -> Vec<(Distance, T)> {
    let mut results = Vec::new();
    for (pt, data) in input {
        let projected = pl.project_pt(*pt);
        // TODO Check distance isn't too crazy? Not sure why it would be
        if let Some((dist, _)) = pl.dist_along_of_point(projected) {
            results.push((dist, data.clone()));
        }
    }
    results
}
