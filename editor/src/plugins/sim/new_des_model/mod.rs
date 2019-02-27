mod events;
mod make;
mod mechanics;
mod query;
mod router;
mod scheduler;
mod sim;
mod trips;

pub use self::events::Event;
pub use self::make::{
    load, ABTest, ABTestResults, BorderSpawnOverTime, OriginDestination, Scenario, SeedParkedCars,
    SimFlags, SpawnOverTime, TripSpawner, TripSpec,
};
pub use self::mechanics::{
    DrivingSimState, IntersectionSimState, ParkingSimState, WalkingSimState,
};
pub use self::query::{Benchmark, ScoreSummary, Summary};
pub use self::router::{ActionAtEnd, Router};
pub use self::scheduler::{Command, Scheduler};
pub use self::sim::Sim;
pub use self::trips::{TripLeg, TripManager};
use ::sim::{CarID, PedestrianID, TripID, VehicleType};
use geom::{Distance, Duration, Speed};
use map_model::{BuildingID, BusStopID, IntersectionID, LaneID, LaneType, Map, Path, Position};
use serde_derive::{Deserialize, Serialize};

// http://pccsc.net/bicycle-parking-info/ says 68 inches, which is 1.73m
pub const MIN_BIKE_LENGTH: Distance = Distance::const_meters(1.7);
pub const MAX_BIKE_LENGTH: Distance = Distance::const_meters(2.0);
// These two must be < PARKING_SPOT_LENGTH
pub const MIN_CAR_LENGTH: Distance = Distance::const_meters(4.5);
pub const MAX_CAR_LENGTH: Distance = Distance::const_meters(6.5);
// Note this is more than MAX_CAR_LENGTH
pub const BUS_LENGTH: Distance = Distance::const_meters(12.5);

// At all speeds (including at rest), cars must be at least this far apart, measured from front of
// one car to the back of the other.
pub const FOLLOWING_DISTANCE: Distance = Distance::const_meters(1.0);

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Vehicle {
    pub id: CarID,
    pub vehicle_type: VehicleType,
    pub length: Distance,
    pub max_speed: Option<Speed>,
}

// TODO Dedupe...
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct VehicleSpec {
    pub vehicle_type: VehicleType,
    pub length: Distance,
    pub max_speed: Option<Speed>,
}

impl VehicleSpec {
    pub fn make(self, id: CarID) -> Vehicle {
        Vehicle {
            id,
            vehicle_type: self.vehicle_type,
            length: self.length,
            max_speed: self.max_speed,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ParkingSpot {
    pub lane: LaneID,
    pub idx: usize,
}

impl ParkingSpot {
    pub fn new(lane: LaneID, idx: usize) -> ParkingSpot {
        ParkingSpot { lane, idx }
    }
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct ParkedCar {
    pub vehicle: Vehicle,
    pub spot: ParkingSpot,
    pub owner: Option<BuildingID>,
}

impl ParkedCar {
    pub fn new(vehicle: Vehicle, spot: ParkingSpot, owner: Option<BuildingID>) -> ParkedCar {
        assert_eq!(vehicle.vehicle_type, VehicleType::Car);
        ParkedCar {
            vehicle,
            spot,
            owner,
        }
    }

    pub fn get_driving_pos(&self, parking: &ParkingSimState, map: &Map) -> Position {
        parking.spot_to_driving_pos(self.spot, &self.vehicle, map)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DrivingGoal {
    ParkNear(BuildingID),
    Border(IntersectionID, LaneID),
}

impl DrivingGoal {
    pub fn goal_pos(&self, map: &Map) -> Position {
        let lane = match self {
            DrivingGoal::ParkNear(b) => map.find_driving_lane_near_building(*b),
            DrivingGoal::Border(_, l) => *l,
        };
        Position::new(lane, map.get_l(lane).length())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct SidewalkSpot {
    pub connection: SidewalkPOI,
    pub sidewalk_pos: Position,
}

impl SidewalkSpot {
    pub fn parking_spot(
        spot: ParkingSpot,
        map: &Map,
        parking_sim: &ParkingSimState,
    ) -> SidewalkSpot {
        // TODO Consider precomputing this.
        let sidewalk = map
            .find_closest_lane(spot.lane, vec![LaneType::Sidewalk])
            .unwrap();
        SidewalkSpot {
            connection: SidewalkPOI::ParkingSpot(spot),
            sidewalk_pos: parking_sim.spot_to_sidewalk_pos(spot, sidewalk, map),
        }
    }

    pub fn building(bldg: BuildingID, map: &Map) -> SidewalkSpot {
        let front_path = &map.get_b(bldg).front_path;
        SidewalkSpot {
            connection: SidewalkPOI::Building(bldg),
            sidewalk_pos: front_path.sidewalk,
        }
    }

    pub fn bike_rack(sidewalk: LaneID, map: &Map) -> Option<SidewalkSpot> {
        assert!(map.get_l(sidewalk).is_sidewalk());
        let driving_lane = map.get_parent(sidewalk).sidewalk_to_bike(sidewalk)?;
        // TODO Arbitrary, but safe
        let sidewalk_pos = Position::new(sidewalk, map.get_l(sidewalk).length() / 2.0);
        let driving_pos = sidewalk_pos.equiv_pos(driving_lane, map);
        Some(SidewalkSpot {
            connection: SidewalkPOI::BikeRack(driving_pos),
            sidewalk_pos,
        })
    }

    pub fn bus_stop(stop: BusStopID, map: &Map) -> SidewalkSpot {
        SidewalkSpot {
            sidewalk_pos: map.get_bs(stop).sidewalk_pos,
            connection: SidewalkPOI::BusStop(stop),
        }
    }

    pub fn start_at_border(i: IntersectionID, map: &Map) -> Option<SidewalkSpot> {
        let lanes = map.get_i(i).get_outgoing_lanes(map, LaneType::Sidewalk);
        if lanes.is_empty() {
            None
        } else {
            Some(SidewalkSpot {
                sidewalk_pos: Position::new(lanes[0], Distance::ZERO),
                connection: SidewalkPOI::Border(i),
            })
        }
    }

    pub fn end_at_border(i: IntersectionID, map: &Map) -> Option<SidewalkSpot> {
        let lanes = map.get_i(i).get_incoming_lanes(map, LaneType::Sidewalk);
        if lanes.is_empty() {
            None
        } else {
            Some(SidewalkSpot {
                sidewalk_pos: Position::new(lanes[0], map.get_l(lanes[0]).length()),
                connection: SidewalkPOI::Border(i),
            })
        }
    }
}

// Point of interest, that is
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum SidewalkPOI {
    ParkingSpot(ParkingSpot),
    Building(BuildingID),
    BusStop(BusStopID),
    Border(IntersectionID),
    // The equivalent position on the nearest driving/bike lane
    BikeRack(Position),
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct TimeInterval {
    // TODO Private fields
    pub start: Duration,
    pub end: Duration,
}

impl TimeInterval {
    pub fn new(start: Duration, end: Duration) -> TimeInterval {
        if end < start {
            panic!("Bad TimeInterval {} .. {}", start, end);
        }
        TimeInterval { start, end }
    }

    pub fn percent(&self, t: Duration) -> f64 {
        if self.start == self.end {
            return 1.0;
        }

        let x = (t - self.start) / (self.end - self.start);
        assert!(x >= 0.0 && x <= 1.0);
        x
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct DistanceInterval {
    // TODO Private fields
    pub start: Distance,
    pub end: Distance,
}

impl DistanceInterval {
    pub fn new_driving(start: Distance, end: Distance) -> DistanceInterval {
        if end < start {
            panic!("Bad DistanceInterval {} .. {}", start, end);
        }
        DistanceInterval { start, end }
    }

    pub fn new_walking(start: Distance, end: Distance) -> DistanceInterval {
        // start > end is fine, might be contraflow.
        DistanceInterval { start, end }
    }

    pub fn lerp(&self, x: f64) -> Distance {
        assert!(x >= 0.0 && x <= 1.0);
        self.start + x * (self.end - self.start)
    }

    pub fn length(&self) -> Distance {
        (self.end - self.start).abs()
    }
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct CreatePedestrian {
    pub id: PedestrianID,
    pub start: SidewalkSpot,
    pub goal: SidewalkSpot,
    pub path: Path,
    pub trip: TripID,
}

#[derive(Serialize, Deserialize, PartialEq, Clone, Debug)]
pub struct CreateCar {
    pub vehicle: Vehicle,
    pub router: Router,
    pub start_dist: Distance,
    pub maybe_parked_car: Option<ParkedCar>,
    pub trip: TripID,
}

impl CreateCar {
    pub fn for_appearing(
        vehicle: Vehicle,
        start_pos: Position,
        router: Router,
        trip: TripID,
    ) -> CreateCar {
        CreateCar {
            vehicle,
            router,
            start_dist: start_pos.dist_along(),
            maybe_parked_car: None,
            trip,
        }
    }

    pub fn for_parked_car(
        parked_car: ParkedCar,
        router: Router,
        trip: TripID,
        parking: &ParkingSimState,
        map: &Map,
    ) -> CreateCar {
        CreateCar {
            vehicle: parked_car.vehicle.clone(),
            router,
            start_dist: parked_car.get_driving_pos(parking, map).dist_along(),
            maybe_parked_car: Some(parked_car),
            trip,
        }
    }
}
