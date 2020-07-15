use crate::make::match_points_to_lanes;
use crate::raw::{RawBusRoute, RawBusStop};
use crate::{
    BusRoute, BusRouteID, BusStop, BusStopID, LaneType, Map, PathConstraints, PathRequest, Position,
};
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
        let mut stops = Vec::new();
        let mut ok = true;
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
                    timer.warn(format!(
                        "Couldn't match stop {} for route {} ({}): {}",
                        stop.name,
                        r.full_name,
                        rel_url(r.osm_rel_id),
                        err,
                    ));
                    ok = false;
                    break;
                }
            }
        }
        if !ok {
            continue;
        }

        // Make sure the stops are connected
        let route_type = if r.is_bus {
            PathConstraints::Bus
        } else {
            PathConstraints::Train
        };
        let mut ok = true;
        for pair in stops.windows(2) {
            if let Err(err) = check_stops(route_type, pair[0], pair[1], map) {
                timer.warn(format!(
                    "Route {} ({}) disconnected: {}",
                    r.full_name,
                    rel_url(r.osm_rel_id),
                    err
                ));
                ok = false;
                break;
            }
        }
        if ok {
            map.bus_routes.push(BusRoute {
                id: BusRouteID(map.bus_routes.len()),
                full_name: r.full_name.clone(),
                short_name: r.short_name.clone(),
                stops,
                route_type,
            });
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

struct Matcher {
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
            Distance::meters(30.0),
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
        is_bus: bool,
        stop: &RawBusStop,
        map: &Map,
    ) -> Result<(Position, Position), Box<dyn Error>> {
        if !is_bus {
            // Light rail needs explicit platforms.
            let sidewalk_pos = *self
                .sidewalk_pts
                .get(
                    &stop
                        .ped_pos
                        .ok_or("light rail missing platform")?
                        .to_hashable(),
                )
                .ok_or("sidewalk didnt match")?;
            let driving_pos = *self
                .light_rail_pts
                .get(&stop.vehicle_pos.to_hashable())
                .ok_or("driving didnt match")?;
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

        // We only have the driving position. First find the sidewalk, then snap it to the
        // rightmost driving/bus lane.
        let orig_driving_pos = *self
            .bus_pts
            .get(&stop.vehicle_pos.to_hashable())
            .ok_or("driving didnt match")?;
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

fn check_stops(
    constraints: PathConstraints,
    stop1: BusStopID,
    stop2: BusStopID,
    map: &Map,
) -> Result<(), Box<dyn Error>> {
    let start = map.get_bs(stop1).driving_pos;
    let end = map.get_bs(stop2).driving_pos;
    if start.lane() == end.lane() && start.dist_along() > end.dist_along() {
        return Err(format!(
            "Two stops seemingly out of order somewhere on {}",
            map.get_parent(start.lane()).orig_id
        )
        .into());
    }

    if map
        .pathfind(PathRequest {
            start,
            end,
            constraints,
        })
        .is_some()
    {
        return Ok(());
    }
    Err(format!(
        "No path between stop on {} and {}",
        map.get_parent(start.lane()).orig_id,
        map.get_parent(end.lane()).orig_id
    )
    .into())
}

fn rel_url(id: i64) -> String {
    format!("https://www.openstreetmap.org/relation/{}", id)
}
