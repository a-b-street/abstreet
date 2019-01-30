use geom::{Duration, Speed, Distance, Pt2D};
use map_model::{BuildingID, BusRouteID, BusStopID, LaneID, Map, TurnID};
use crate::{CarID, ParkingSpot};

// This is experimental for now, but it might subsume the entire design of the sim crate.

// TODO From a conversation with Julian: have to prune the search dramatically. After hopping in a
// car, we do have the option of parking anywhere, but we really only want to try to park close to
// the goal, which we can't plan in advance anyway.
//
// Possibly want a high- and low-level plan. The high-level one roughly plans TripLegs.

// TODO It's very tempting to have a different action for each modality. CrossDrivingLane,
// CrossSidewalk.
enum Action {
    // Cars (including buses) and pedestrians
    CrossLane(LaneID),
    CrossTurn(TurnID),

    // Only cars
    ParkingCar(CarID, ParkingSpot),
    UnparkingCar(CarID, ParkingSpot),
    // TODO Lanechanging

    // Only pedestrians
    CrossLaneContraflow(LaneID),
    CrossPathFromBuildingToSidewalk(BuildingID),
    CrossPathFromSidewalkToBuilding(BuildingID),
    WaitForBus(BusStopID, BusRouteID),
    // (from, to)
    RideBus(BusStopID, BusStopID),
    // TODO parking, unparking bike

    // Only buses
    DeboardPassengers(BusStopID),
    BoardPassengers(BusStopID),
}

impl Action {
    // These are always lower bounds, aka, the best case.
    fn cost(&self, map: &Map) -> Duration {
        // TODO driving speed limits and these could depend on preferences of the individual
        // ped/vehicle
        let ped_speed = Speed::meters_per_second(3.9);

        match *self {
            // TODO wait, we need to know if a ped or car is crossing something
            Action::CrossLane(id) => map.get_l(id).length() / map.get_parent(id).get_speed_limit(),
            Action::CrossTurn(id) => {
                map.get_t(id).length() / map.get_parent(id.dst).get_speed_limit()
            }
            Action::ParkingCar(_, _) => Duration::seconds(20.0),
            Action::UnparkingCar(_, _) => Duration::seconds(10.0),
            Action::CrossLaneContraflow(id) => map.get_l(id).length() / ped_speed,
            Action::CrossPathFromBuildingToSidewalk(id)
            | Action::CrossPathFromSidewalkToBuilding(id) => {
                map.get_b(id).front_path.line.length() / ped_speed
            }
            // TODO Could try lots of things here...
            Action::WaitForBus(_, _) => Duration::seconds(60.0),
            // TODO Cache the expected time to travel between stops
            Action::RideBus(_stop1, _stop2) => Duration::seconds(300.0),

            _ => panic!("TODO"),
        }
    }

    // After completing this action, how far will we be from the goal?
    // Does this need to be admissible?
    // TODO hard to convert distance and time
    fn heuristic(&self, goal: Pt2D) -> Distance {
        // TODO
        Distance::ZERO
    }

    fn next_steps(&self) -> Vec<Action> {
        // TODO
        Vec::new()
    }
}
