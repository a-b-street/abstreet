use crate::make::match_points_to_lanes;
use crate::raw::{RawBusRoute, RawBusStop};
use crate::{
    BusRoute, BusRouteID, BusStop, BusStopID, LaneType, Map, PathConstraints, PathRequest, Position,
};
use abstutil::Timer;
use geom::{Distance, HashablePt2D};
use std::collections::{BTreeMap, HashMap, HashSet};

struct Matcher {
    sidewalk_pts: HashMap<HashablePt2D, Position>,
    bus_pts: HashMap<HashablePt2D, Position>,
    light_rail_pts: HashMap<HashablePt2D, Position>,
}

impl Matcher {
    fn new(bus_routes: &Vec<RawBusRoute>, map: &Map, timer: &mut Timer) -> Matcher {
        // Match all of the points to an exact position along a lane.
        let mut lookup_sidewalk_pts = HashSet::new();
        let mut lookup_bus_pts = HashSet::new();
        let mut lookup_light_rail_pts = HashSet::new();
        for r in bus_routes {
            for stop in &r.fwd_stops {
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
            Distance::meters(10.0),
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
    fn lookup(&self, is_bus: bool, stop: &RawBusStop, map: &Map) -> Option<(Position, Position)> {
        let driving_pos = if is_bus {
            self.bus_pts.get(&stop.vehicle_pos.to_hashable())?
        } else {
            self.light_rail_pts.get(&stop.vehicle_pos.to_hashable())?
        };
        // Explicit platform?
        if let Some(pt) = stop.ped_pos {
            let sidewalk_pos = self.sidewalk_pts.get(&pt.to_hashable())?;
            return Some((*sidewalk_pos, *driving_pos));
        }
        if !is_bus {
            // Light rail needs explicit platforms
            return None;
        }

        // Manually snap a bus position to a sidewalk
        let sidewalk = map
            .get_parent(driving_pos.lane())
            .find_closest_lane(driving_pos.lane(), vec![LaneType::Sidewalk])
            .ok()?;
        let sidewalk_pos = driving_pos.equiv_pos(sidewalk, Distance::ZERO, map);
        Some((sidewalk_pos, *driving_pos))
    }
}

pub fn make_bus_stops(
    map: &mut Map,
    bus_routes: &Vec<RawBusRoute>,
    timer: &mut Timer,
) -> (BTreeMap<BusStopID, BusStop>, Vec<BusRoute>) {
    timer.start("make bus stops");
    let matcher = Matcher::new(bus_routes, map, timer);

    // TODO I'm assuming the vehicle_pos <-> driving_pos relation is one-to-one...
    let mut pt_to_stop: BTreeMap<(Position, Position), BusStopID> = BTreeMap::new();
    let mut bus_stops: BTreeMap<BusStopID, BusStop> = BTreeMap::new();
    let mut routes: Vec<BusRoute> = Vec::new();

    for r in bus_routes {
        let mut stops = Vec::new();
        for stop in &r.fwd_stops {
            if let Some((sidewalk_pos, driving_pos)) = matcher.lookup(r.is_bus, stop, map) {
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
                    bus_stops.insert(
                        id,
                        BusStop {
                            id,
                            name: stop.name.clone(),
                            driving_pos,
                            sidewalk_pos,
                        },
                    );
                    id
                };
                stops.push(stop_id);
            }
        }
        routes.push(BusRoute {
            id: BusRouteID(routes.len()),
            name: r.name.clone(),
            stops,
        });
    }

    timer.stop("make bus stops");
    (bus_stops, routes)
}

// Trim out stops if needed; map borders sometimes mean some paths don't work.
pub fn fix_bus_route(map: &Map, r: &mut BusRoute) -> bool {
    let mut stops = Vec::new();
    for stop in r.stops.drain(..) {
        if stops.is_empty() {
            stops.push(stop);
        } else {
            if check_stops(*stops.last().unwrap(), stop, map) {
                stops.push(stop);
            }
        }
    }
    // Don't forget the last and first
    while stops.len() >= 2 {
        if check_stops(*stops.last().unwrap(), stops[0], map) {
            break;
        }
        // TODO Or the front one
        stops.pop();
    }
    r.stops = stops;
    r.stops.len() >= 2
}

fn check_stops(stop1: BusStopID, stop2: BusStopID, map: &Map) -> bool {
    let bs1 = map.get_bs(stop1);
    let bs2 = map.get_bs(stop2);
    // This is coming up because the dist_along's are in a bad order. But why should
    // this happen at all?
    let ok1 = bs1.driving_pos.lane() != bs2.driving_pos.lane();
    let ok2 = map
        .pathfind(PathRequest {
            start: bs1.driving_pos,
            end: bs2.driving_pos,
            constraints: PathConstraints::Bus,
        })
        .is_some();
    ok1 && ok2
}
