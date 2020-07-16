use crate::make::match_points_to_lanes;
use crate::raw::{RawBusRoute, RawBusStop};
use crate::{BusRoute, BusRouteID, BusStop, BusStopID, LaneType, Map, PathConstraints, Position};
use abstutil::Timer;
use geom::{Distance, HashablePt2D};
use std::collections::{BTreeMap, HashMap, HashSet};
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
                r.full_name,
                rel_url(r.osm_rel_id),
                err
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
    let mut stops = Vec::new();
    for stop in &r.stops {
        match matcher.lookup(r.is_bus, stop, map) {
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

    let route_type = if r.is_bus {
        PathConstraints::Bus
    } else {
        PathConstraints::Train
    };

    // Start or end at a border?
    let mut start_border = None;
    let mut end_border = None;
    if let Some(i) = r.border_start {
        let i = map.get_i(map.find_i_by_osm_id(i.osm_node_id).unwrap());
        if !i.is_border() {
            panic!("Route starts at {}, but isn't a border?", i.orig_id);
        }
        if let Some(l) = i.get_outgoing_lanes(map, route_type).get(0) {
            start_border = Some(*l);
        } else {
            // TODO Should panic
            println!(
                "Route {} starts at {} ({}), but no starting lane for a {:?}?",
                rel_url(r.osm_rel_id),
                i.id,
                i.orig_id,
                route_type
            );
        }
    }
    if let Some(i) = r.border_end {
        let i = map.get_i(map.find_i_by_osm_id(i.osm_node_id).unwrap());
        if !i.is_border() {
            panic!("Route ends at {}, but isn't a border?", i.orig_id);
        }
        if let Some(l) = i.get_incoming_lanes(map, route_type).next() {
            end_border = Some(l);
        } else {
            // TODO Should panic
            println!(
                "Route {} ends at {} ({}), but no ending lane for a {:?}?",
                rel_url(r.osm_rel_id),
                i.id,
                i.orig_id,
                route_type
            );
        }
    }

    let route = BusRoute {
        id: BusRouteID(map.bus_routes.len()),
        full_name: r.full_name.clone(),
        short_name: r.short_name.clone(),
        stops,
        route_type,
        start_border,
        end_border,
    };

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
                "No path between stop on {} and {}",
                map.get_parent(req.start.lane()).orig_id,
                map.get_parent(req.end.lane()).orig_id
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
            |l| l.is_sidewalk(),
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
            // TODO Buffer?
            Distance::ZERO,
            Distance::meters(10.0),
            timer,
        );
        let light_rail_pts = match_points_to_lanes(
            map.get_bounds(),
            lookup_light_rail_pts,
            map.all_lanes(),
            |l| l.lane_type == LaneType::LightRail,
            // TODO Buffer?
            Distance::ZERO,
            Distance::meters(20.0),
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
        is_bus: bool,
        stop: &RawBusStop,
        map: &Map,
    ) -> Result<(Position, Position), Box<dyn Error>> {
        if !is_bus {
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
        if let Some(pt) = stop.ped_pos {
            let sidewalk_pos = *self
                .sidewalk_pts
                .get(&pt.to_hashable())
                .ok_or("sidewalk didnt match")?;
            let lane = map
                .get_parent(sidewalk_pos.lane())
                .find_closest_lane(sidewalk_pos.lane(), vec![LaneType::Bus, LaneType::Driving])?;
            let driving_pos = sidewalk_pos.equiv_pos(lane, Distance::ZERO, map);
            return Ok((sidewalk_pos, driving_pos));
        }

        // We only have the vehicle position. First find the sidewalk, then snap it to the
        // rightmost driving/bus lane.
        let orig_driving_pos = *self
            .bus_pts
            .get(&stop.vehicle_pos.to_hashable())
            .ok_or("vehicle for bus didnt match")?;
        let sidewalk = map
            .get_parent(orig_driving_pos.lane())
            .find_closest_lane(orig_driving_pos.lane(), vec![LaneType::Sidewalk])?;
        let sidewalk_pos = orig_driving_pos.equiv_pos(sidewalk, Distance::ZERO, map);
        let lane = map
            .get_parent(sidewalk_pos.lane())
            .find_closest_lane(sidewalk_pos.lane(), vec![LaneType::Bus, LaneType::Driving])?;
        let driving_pos = sidewalk_pos.equiv_pos(lane, Distance::ZERO, map);
        Ok((sidewalk_pos, driving_pos))
    }
}

fn rel_url(id: i64) -> String {
    format!("https://www.openstreetmap.org/relation/{}", id)
}
