use crate::simplified::{Outcome, VehiclePathfinder};
use crate::walking::SidewalkPathfinder;
use map_model::{BusRouteID, BusStopID, LaneType, Map, Path, PathRequest, Position};

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
            bike_graph: VehiclePathfinder::new(map, vec![LaneType::Driving, LaneType::Biking]),
            bus_graph: VehiclePathfinder::new(map, vec![LaneType::Driving, LaneType::Bus]),
            walking_graph: SidewalkPathfinder::new(map, false),
            walking_with_transit_graph: SidewalkPathfinder::new(map, true),
        }
    }

    pub fn pathfind(&self, req: PathRequest, map: &Map) -> Option<Path> {
        if req.start == req.end {
            panic!("Bad request {:?}", req);
        }

        let outcome = if map.get_l(req.start.lane()).is_sidewalk() {
            match self.walking_graph.pathfind(&req, map) {
                Some(path) => Outcome::Success(path),
                None => Outcome::Failure,
            }
        } else if req.can_use_bus_lanes {
            self.bus_graph.pathfind(&req, map)
        } else if req.can_use_bike_lanes {
            self.bike_graph.pathfind(&req, map)
        } else {
            self.car_graph.pathfind(&req, map)
        };
        match outcome {
            //Outcome::Success(path) => Some(path),
            Outcome::Success(path) => {
                let ok1 = match path.current_step().as_traversable() {
                    map_model::Traversable::Lane(l) => l == req.start.lane(),
                    map_model::Traversable::Turn(t) => t.src == req.start.lane(),
                };
                let ok2 = match path.last_step().as_traversable() {
                    map_model::Traversable::Lane(l) => l == req.end.lane(),
                    map_model::Traversable::Turn(t) => t.dst == req.end.lane(),
                };
                if !ok1 || !ok2 {
                    println!("request is {:?}", req);
                    for step in path.get_steps() {
                        println!("- {:?}", step);
                    }
                    panic!(
                        "bad path starting on a {:?}",
                        map.get_l(req.start.lane()).lane_type
                    );
                }

                Some(path)
            }
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
        self.walking_with_transit_graph
            .should_use_transit(map, start, end)
    }
}
