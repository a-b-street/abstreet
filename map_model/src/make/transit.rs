use std::collections::{HashMap, HashSet};

use anyhow::Result;

use abstutil::Timer;
use geom::{Distance, Duration, FindClosest, HashablePt2D, Time};

use crate::make::match_points_to_lanes;
use crate::raw::{RawMap, RawTransitRoute, RawTransitStop, RawTransitType};
use crate::{
    LaneID, Map, PathConstraints, Position, TransitRoute, TransitRouteID, TransitStop,
    TransitStopID,
};

pub fn finalize_transit(map: &mut Map, raw: &RawMap, timer: &mut Timer) {
    // Snap stops to sidewalks and driving lanes, similar to buildings
    let mut query: HashSet<HashablePt2D> = HashSet::new();
    for stop in raw.transit_stops.values() {
        query.insert(stop.position.to_hashable());
    }
    let sidewalk_pts = match_points_to_lanes(
        map,
        query,
        |l| l.is_walkable(),
        // Stops can be very close to intersections
        Distance::ZERO,
        // Stops shouldn't be far from sidewalks
        Distance::meters(3.0),
        timer,
    );

    // Create all stops
    let mut gtfs_to_stop_id: HashMap<String, TransitStopID> = HashMap::new();
    for stop in raw.transit_stops.values() {
        if let Err(err) = create_stop(stop, &sidewalk_pts, &mut gtfs_to_stop_id, map) {
            warn!("Couldn't create stop {}: {}", stop.gtfs_id, err);
        }
    }

    let snapper = BorderSnapper::new(map);
    for route in &raw.transit_routes {
        if let Err(err) = create_route(route, map, &gtfs_to_stop_id, &snapper) {
            warn!(
                "Couldn't snap route {} ({}): {}",
                route.gtfs_id, route.short_name, err
            );
        }
    }

    // TODO Clean up unused stops; maybe one of the routes didn't work. Re-map IDs...
}

fn create_stop(
    stop: &RawTransitStop,
    sidewalk_pts: &HashMap<HashablePt2D, Position>,
    gtfs_to_stop_id: &mut HashMap<String, TransitStopID>,
    map: &mut Map,
) -> Result<()> {
    // TODO We'll have to look up all routes referencing this stop and determine this
    let vehicle = PathConstraints::Bus;
    if let Some(sidewalk_pos) = sidewalk_pts.get(&stop.position.to_hashable()) {
        let sidewalk_lane = sidewalk_pos.lane();
        if let Some(driving_pos) = map
            .get_parent(sidewalk_lane)
            .find_closest_lane(sidewalk_lane, |l| vehicle.can_use(l, map))
            .map(|l| sidewalk_pos.equiv_pos(l, map))
        {
            let road = sidewalk_lane.road;
            let id = TransitStopID {
                road,
                idx: map.get_r(road).transit_stops.len(),
            };
            map.mut_road(road).transit_stops.insert(id);
            map.transit_stops.insert(
                id,
                TransitStop {
                    id,
                    name: stop.name.clone(),
                    gtfs_id: stop.gtfs_id.clone(),
                    driving_pos,
                    sidewalk_pos: *sidewalk_pos,
                    is_train_stop: vehicle == PathConstraints::Train,
                },
            );
            gtfs_to_stop_id.insert(stop.gtfs_id.clone(), id);
            Ok(())
        } else {
            bail!(
                "Couldn't find a lane for {:?} next to sidewalk {}",
                vehicle,
                sidewalk_lane
            );
        }
    } else {
        bail!("Stop position {} wasn't close to a sidewalk", stop.position);
    }
}

struct BorderSnapper {
    bus_incoming_borders: FindClosest<LaneID>,
    bus_outgoing_borders: FindClosest<LaneID>,
    train_incoming_borders: FindClosest<LaneID>,
    train_outgoing_borders: FindClosest<LaneID>,
}

impl BorderSnapper {
    fn new(map: &Map) -> BorderSnapper {
        let mut snapper = BorderSnapper {
            bus_incoming_borders: FindClosest::new(map.get_bounds()),
            bus_outgoing_borders: FindClosest::new(map.get_bounds()),
            train_incoming_borders: FindClosest::new(map.get_bounds()),
            train_outgoing_borders: FindClosest::new(map.get_bounds()),
        };
        for i in map.all_incoming_borders() {
            for l in i.get_outgoing_lanes(map, PathConstraints::Bus) {
                // TODO FindClosest doesn't handle single points as geometries, so use the lane
                // polygon
                snapper
                    .bus_incoming_borders
                    .add(l, map.get_l(l).get_thick_polygon().points());
            }
            for l in i.get_outgoing_lanes(map, PathConstraints::Train) {
                snapper
                    .train_incoming_borders
                    .add(l, map.get_l(l).get_thick_polygon().points());
            }
        }
        for i in map.all_outgoing_borders() {
            for l in i.get_incoming_lanes(map, PathConstraints::Bus) {
                snapper
                    .bus_outgoing_borders
                    .add(l, map.get_l(l).get_thick_polygon().points());
            }
            for l in i.get_incoming_lanes(map, PathConstraints::Train) {
                snapper
                    .train_outgoing_borders
                    .add(l, map.get_l(l).get_thick_polygon().points());
            }
        }
        snapper
    }
}

fn create_route(
    route: &RawTransitRoute,
    map: &mut Map,
    gtfs_to_stop_id: &HashMap<String, TransitStopID>,
    snapper: &BorderSnapper,
) -> Result<()> {
    // TODO At least warn about stops that failed to snap
    let stops: Vec<TransitStopID> = route
        .stops
        .iter()
        .filter_map(|gtfs_id| gtfs_to_stop_id.get(gtfs_id).cloned())
        .collect();
    if stops.is_empty() {
        bail!("No valid stops");
    }
    let border_snap_threshold = Distance::meters(30.0);

    let start = if map.boundary_polygon.contains_pt(route.shape.first_pt()) {
        map.get_ts(stops[0]).driving_pos.lane()
    } else {
        // Find the first time the route shape hits the map boundary
        let entry_pt = *map
            .boundary_polygon
            .clone()
            .into_ring()
            .all_intersections(&route.shape)
            .get(0)
            .ok_or_else(|| anyhow!("couldn't find where shape enters map"))?;
        // Snap that to a border
        let borders = if route.route_type == RawTransitType::Bus {
            &snapper.bus_incoming_borders
        } else {
            &snapper.train_incoming_borders
        };
        match borders.closest_pt(entry_pt, border_snap_threshold) {
            Some((l, _)) => l,
            None => bail!(
                "Couldn't find a {:?} border near start {}",
                route.route_type,
                entry_pt
            ),
        }
    };

    let end_border = if map.boundary_polygon.contains_pt(route.shape.last_pt()) {
        None
    } else {
        // Find the last time the route shape hits the map boundary
        let exit_pt = *map
            .boundary_polygon
            .clone()
            .into_ring()
            .all_intersections(&route.shape)
            .last()
            .ok_or_else(|| anyhow!("couldn't find where shape leaves map"))?;
        // Snap that to a border
        let borders = if route.route_type == RawTransitType::Bus {
            &snapper.bus_outgoing_borders
        } else {
            &snapper.train_outgoing_borders
        };
        match borders.closest_pt(exit_pt, border_snap_threshold) {
            Some((lane, _)) => {
                // Edge case: the last stop is on the same road as the border. We can't lane-change
                // suddenly, so match the lane in that case.
                let last_stop_lane = map.get_ts(*stops.last().unwrap()).driving_pos.lane();
                Some(if lane.road == last_stop_lane.road {
                    last_stop_lane
                } else {
                    lane
                })
            }
            None => bail!(
                "Couldn't find a {:?} border near end {}",
                route.route_type,
                exit_pt
            ),
        }
    };

    // TODO This'll come from the RawTransitRoute eventually. For now, every 30 minutes.
    let spawn_times: Vec<Time> = (0..48)
        .map(|i| Time::START_OF_DAY + (i as f64) * Duration::minutes(30))
        .collect();

    let result = TransitRoute {
        id: TransitRouteID(map.transit_routes.len()),
        long_name: route.long_name.clone(),
        short_name: route.short_name.clone(),
        gtfs_id: route.gtfs_id.clone(),
        stops,
        start,
        end_border,
        route_type: match route.route_type {
            RawTransitType::Bus => PathConstraints::Bus,
            RawTransitType::Train => PathConstraints::Train,
        },
        spawn_times: spawn_times.clone(),
        orig_spawn_times: spawn_times,
    };

    // Check that the paths are valid
    result.all_paths(map)?;

    map.transit_routes.push(result);
    Ok(())
}
