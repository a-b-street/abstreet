use crate::make::match_points_to_lanes;
use crate::raw::{RawBusRoute, RawBusStop};
use crate::{
    BusRoute, BusRouteID, BusStop, BusStopID, LaneID, LaneType, Map, PathConstraints, Position,
};
use abstutil::Timer;
use geom::{Distance, Duration, HashablePt2D, Time};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::error::Error;

pub fn make_stops_and_routes(map: &mut Map, raw_routes: &Vec<RawBusRoute>, timer: &mut Timer) {
    timer.start("make transit stops and routes");
    let matcher = Matcher::new(raw_routes, map, timer);

    // TODO I'm assuming the vehicle_pos <-> driving_pos relation is one-to-one...
    let mut pt_to_stop: BTreeMap<(Position, Position), BusStopID> = BTreeMap::new();
    for r in raw_routes {
        if let Err(err) = make_route(map, r, &mut pt_to_stop, &matcher) {
            timer.warn(format!(
                "Skipping route {} ({}): {}",
                r.full_name, r.osm_rel_id, err
            ));
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

    timer.stop("make transit stops and routes");
}

fn make_route(
    map: &mut Map,
    r: &RawBusRoute,
    pt_to_stop: &mut BTreeMap<(Position, Position), BusStopID>,
    matcher: &Matcher,
) -> Result<(), Box<dyn Error>> {
    let route_type = if r.is_bus {
        PathConstraints::Bus
    } else {
        PathConstraints::Train
    };

    let mut stops = Vec::new();
    for stop in &r.stops {
        match matcher.lookup(route_type, stop, map) {
            Ok((sidewalk_pos, driving_pos)) => {
                // Create a new bus stop if needed.
                let stop_id = if let Some(id) = pt_to_stop.get(&(sidewalk_pos, driving_pos)) {
                    *id
                } else {
                    let id = BusStopID {
                        sidewalk: sidewalk_pos.lane(),
                        idx: map.get_l(sidewalk_pos.lane()).bus_stops.len(),
                    };
                    pt_to_stop.insert((sidewalk_pos, driving_pos), id);
                    map.lanes[sidewalk_pos.lane().0].bus_stops.insert(id);
                    map.bus_stops.insert(
                        id,
                        BusStop {
                            id,
                            name: stop.name.clone(),
                            driving_pos,
                            sidewalk_pos,
                            is_train_stop: !r.is_bus,
                        },
                    );
                    id
                };
                stops.push(stop_id);
            }
            Err(err) => {
                return Err(format!("couldn't match stop {}: {}", stop.name, err).into());
            }
        }
    }

    // Start or end at a border?
    let mut end_border = None;
    let start = if let Some(i) = r.border_start {
        let i = map.get_i(map.find_i_by_osm_id(i).unwrap());
        if !i.is_border() {
            panic!("Route starts at {}, but isn't a border?", i.orig_id);
        }
        if let Some(l) = i.get_outgoing_lanes(map, route_type).get(0) {
            *l
        } else {
            return Err(format!(
                "Route {} starts at {} ({}), but no starting lane for a {:?}?",
                r.osm_rel_id, i.id, i.orig_id, route_type
            )
            .into());
        }
    } else {
        // Not starting at a border. Find a lane at or before the first stop that's at least 13m.
        pick_start_lane(map.get_bs(stops[0]).driving_pos, route_type, map)?
    };
    if let Some(i) = r.border_end {
        let i = map.get_i(map.find_i_by_osm_id(i).unwrap());
        if !i.is_border() {
            panic!("Route ends at {}, but isn't a border?", i.orig_id);
        }
        // If the last stop is on a lane leading to the border, don't try to lane-change last
        // minute
        let last_stop_l = map.get_bs(*stops.last().unwrap()).driving_pos.lane();
        if map.get_l(last_stop_l).dst_i == i.id {
            end_border = Some(last_stop_l);
        } else if let Some(l) = i.get_incoming_lanes(map, route_type).next() {
            end_border = Some(l);
        } else {
            // TODO Should panic
            println!(
                "Route {} ends at {} ({}), but no ending lane for a {:?}?",
                r.osm_rel_id, i.id, i.orig_id, route_type
            );
        }
    }

    let route = BusRoute {
        id: BusRouteID(map.bus_routes.len()),
        full_name: r.full_name.clone(),
        short_name: r.short_name.clone(),
        osm_rel_id: r.osm_rel_id,
        gtfs_trip_marker: r.gtfs_trip_marker.clone(),
        stops,
        route_type,
        start,
        end_border,
        spawn_times: default_spawn_times(),
        orig_spawn_times: default_spawn_times(),
    };

    let mut debug_route = format!("All parts of the route:");
    debug_route = format!("{}\nStart at {}", debug_route, route.start);
    for (idx, bs) in route.stops.iter().enumerate() {
        let stop = map.get_bs(*bs);
        debug_route = format!(
            "{}\nStop {} ({}): {}",
            debug_route,
            idx + 1,
            stop.name,
            stop.driving_pos
        );
    }
    if let Some(l) = route.end_border {
        debug_route = format!("{}\nEnd at {}", debug_route, l);
    }

    // Make sure the route is connected
    for req in route.all_steps(map) {
        if req.start.lane() == req.end.lane() && req.start.dist_along() > req.end.dist_along() {
            return Err(format!(
                "Two stops seemingly out of order somewhere on {}",
                map.get_parent(req.start.lane()).orig_id
            )
            .into());
        }

        if map.pathfind(req.clone()).is_none() {
            return Err(format!(
                "No path between stop on {} and {}: {}. {}",
                map.get_parent(req.start.lane()).orig_id,
                map.get_parent(req.end.lane()).orig_id,
                req,
                debug_route
            )
            .into());
        }
    }

    map.bus_routes.push(route);
    Ok(())
}

struct Matcher {
    // TODO Eventually, maybe also map to a station building too
    sidewalk_pts: HashMap<HashablePt2D, Position>,
    bus_pts: HashMap<HashablePt2D, Position>,
    light_rail_pts: HashMap<HashablePt2D, Position>,
}

impl Matcher {
    fn new(routes: &Vec<RawBusRoute>, map: &Map, timer: &mut Timer) -> Matcher {
        // Match all of the points to an exact position along a lane.
        let mut lookup_sidewalk_pts = HashSet::new();
        let mut lookup_bus_pts = HashSet::new();
        let mut lookup_light_rail_pts = HashSet::new();
        for r in routes {
            for stop in &r.stops {
                if r.is_bus {
                    lookup_bus_pts.insert(stop.vehicle_pos.to_hashable());
                } else {
                    lookup_light_rail_pts.insert(stop.vehicle_pos.to_hashable());
                }
                if let Some(pt) = stop.ped_pos {
                    lookup_sidewalk_pts.insert(pt.to_hashable());
                }
            }
        }
        let sidewalk_pts = match_points_to_lanes(
            map.get_bounds(),
            lookup_sidewalk_pts,
            map.all_lanes(),
            |l| l.is_walkable(),
            Distance::ZERO,
            // TODO Generous for cap hill light rail platform
            Distance::meters(50.0),
            timer,
        );
        let bus_pts = match_points_to_lanes(
            map.get_bounds(),
            lookup_bus_pts,
            map.all_lanes(),
            |l| l.is_bus() || l.is_driving(),
            Distance::ZERO,
            Distance::meters(10.0),
            timer,
        );
        let light_rail_pts = match_points_to_lanes(
            map.get_bounds(),
            lookup_light_rail_pts,
            map.all_lanes(),
            |l| l.lane_type == LaneType::LightRail,
            Distance::ZERO,
            Distance::meters(10.0),
            timer,
        );

        Matcher {
            sidewalk_pts,
            bus_pts,
            light_rail_pts,
        }
    }

    // returns (sidewalk, driving)
    fn lookup(
        &self,
        route_type: PathConstraints,
        stop: &RawBusStop,
        map: &Map,
    ) -> Result<(Position, Position), Box<dyn Error>> {
        if route_type == PathConstraints::Train {
            // Light rail needs explicit platforms.
            let sidewalk_pt = stop.ped_pos.ok_or("light rail missing platform")?;
            let sidewalk_pos = *self
                .sidewalk_pts
                .get(&sidewalk_pt.to_hashable())
                .ok_or_else(|| format!("sidewalk for light rail didnt match: {}", sidewalk_pt))?;
            let driving_pos = *self
                .light_rail_pts
                .get(&stop.vehicle_pos.to_hashable())
                .ok_or_else(|| {
                    format!("vehicle for light rail didnt match: {}", stop.vehicle_pos)
                })?;
            return Ok((sidewalk_pos, driving_pos));
        }

        // Because the stop is usually mapped on the road center-line, the matched side-of-the-road
        // is often wrong. If we have the bus stop, actually use that and get the equivalent
        // position on the closest driving/bus lane.
        let sidewalk_pos = if let Some(pt) = stop.ped_pos {
            *self
                .sidewalk_pts
                .get(&pt.to_hashable())
                .ok_or("sidewalk didnt match")?
        } else {
            // We only have the vehicle position. First find the sidewalk, then snap it to the
            // rightmost driving/bus lane.
            let orig_driving_pos = *self
                .bus_pts
                .get(&stop.vehicle_pos.to_hashable())
                .ok_or("vehicle for bus didnt match")?;
            let sidewalk = map
                .get_parent(orig_driving_pos.lane())
                .find_closest_lane(
                    orig_driving_pos.lane(),
                    |l| PathConstraints::Pedestrian.can_use(l, map),
                    map,
                )
                .ok_or_else(|| format!("driving {} to sidewalk failed", orig_driving_pos.lane()))?;
            orig_driving_pos.equiv_pos(sidewalk, map)
        };
        let lane = map
            .get_parent(sidewalk_pos.lane())
            .find_closest_lane(sidewalk_pos.lane(), |l| route_type.can_use(l, map), map)
            .ok_or_else(|| format!("sidewalk {} to driving failed", sidewalk_pos.lane()))?;
        let mut driving_pos = sidewalk_pos.equiv_pos(lane, map);
        // If we're a stop right at an incoming border, make sure to be at least past where the bus
        // will spawn from the border. pick_start_lane() can't do anything for borders.
        if map
            .get_i(map.get_l(driving_pos.lane()).src_i)
            .is_incoming_border()
        {
            if let Some(pos) = driving_pos.min_dist(Distance::meters(1.0), map) {
                driving_pos = pos;
            } else {
                return Err(
                    format!("too close to start of a border {}", driving_pos.lane()).into(),
                );
            }
        }
        Ok((sidewalk_pos, driving_pos))
    }
}

fn pick_start_lane(
    first_stop: Position,
    constraints: PathConstraints,
    map: &Map,
) -> Result<LaneID, String> {
    let min_len = Distance::meters(13.0);
    if first_stop.dist_along() >= min_len {
        return Ok(first_stop.lane());
    }

    // Flood backwards until we find a long enough lane
    let mut queue = VecDeque::new();
    queue.push_back(first_stop.lane());
    while !queue.is_empty() {
        let current = queue.pop_front().unwrap();
        if current != first_stop.lane() && map.get_l(current).length() >= min_len {
            return Ok(current);
        }
        for t in map.get_turns_to_lane(current) {
            if constraints.can_use(map.get_l(t.id.src), map) {
                queue.push_back(t.id.src);
            }
        }
    }
    Err(format!(
        "couldn't find any lanes leading to {} that're long enough for a bus to spawn",
        first_stop.lane()
    ))
}

fn default_spawn_times() -> Vec<Time> {
    // Hourly spawning from midnight to 7, then every 30 minutes till 7, then hourly again
    let mut times = Vec::new();
    for i in 0..24 {
        let hour = Time::START_OF_DAY + Duration::hours(i);
        times.push(hour);
        if i >= 7 && i <= 19 {
            times.push(hour + Duration::minutes(30));
        }
    }
    times
}
