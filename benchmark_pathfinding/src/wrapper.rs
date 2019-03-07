use crate::simplified::{Outcome, VehiclePathfinder};
use crate::walking::SidewalkPathfinder;
use map_model::{LaneType, Map, PathRequest, Path, Position, BusStopID, BusRouteID};

pub struct Pathfinder {
    car_graph: VehiclePathfinder,
    bike_graph: VehiclePathfinder,
    bus_graph: VehiclePathfinder,
    walking_graph: SidewalkPathfinder,
    walking_with_transit_graph: SidewalkPathfinder,
}

impl Pathfinder {
    pub fn new(map: &Map) -> Pathfinder {
        Pathfinder {
            car_graph: VehiclePathfinder::new(map, vec![LaneType::Driving]),
            bike_graph:
                VehiclePathfinder::new(map, vec![LaneType::Driving, LaneType::Biking]),
            bus_graph:
                VehiclePathfinder::new(map, vec![LaneType::Driving, LaneType::Bus]),
            walking_graph: SidewalkPathfinder::new(map, false),
            walking_with_transit_graph: SidewalkPathfinder::new(map, true),
        }
    }

    pub fn pathfind(&self, req: PathRequest, map: &Map) -> Option<Path> {
        if map.get_l(req.start.lane()).is_sidewalk() {
            return self.walking_graph.pathfind(&req, map);
        }
        let outcome = if req.can_use_bus_lanes {
            self.bus_graph.pathfind(&req, map)
        } else if req.can_use_bike_lanes {
            self.bike_graph.pathfind(&req, map)
        } else {
            self.car_graph.pathfind(&req, map)
        };
        match outcome {
            Outcome::Success(path) => Some(path),
            Outcome::Failure => None,
            Outcome::RetrySlow => map_model::Pathfinder::shortest_distance(map, req),
        }
    }

    pub fn should_use_transit(
        &self,
        map: &Map,
        start: Position,
        end: Position,
    ) -> Option<(BusStopID, BusStopID, BusRouteID)> {
        self.walking_with_transit_graph.should_use_transit(map, start, end)
    }
}
