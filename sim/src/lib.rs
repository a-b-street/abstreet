mod events;
mod make;
mod mechanics;
mod render;
mod router;
mod scheduler;
mod sim;
mod transit;
mod trips;

pub use self::events::Event;
pub use self::make::{
    ABTest, BorderSpawnOverTime, OriginDestination, Scenario, SeedParkedCars, SimFlags,
    SpawnOverTime, SpawnTrip, TripSpawner, TripSpec,
};
pub(crate) use self::mechanics::{
    DrivingSimState, IntersectionSimState, ParkingSimState, WalkingSimState,
};
pub(crate) use self::router::{ActionAtEnd, Router};
pub(crate) use self::scheduler::{Command, Scheduler};
pub use self::sim::Sim;
pub(crate) use self::transit::TransitSimState;
pub use self::trips::{FinishedTrips, TripEnd, TripMode, TripStart, TripStatus};
pub(crate) use self::trips::{TripLeg, TripManager};
pub use crate::render::{
    CarStatus, DrawCarInput, DrawPedestrianInput, GetDrawAgents, UnzoomedAgent,
};
use abstutil::Cloneable;
use geom::{Distance, Duration, Pt2D, Speed};
use map_model::{BuildingID, BusStopID, IntersectionID, LaneID, LaneType, Map, Path, Position};
use serde_derive::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

// http://pccsc.net/bicycle-parking-info/ says 68 inches, which is 1.73m
pub const BIKE_LENGTH: Distance = Distance::const_meters(1.8);
// These two must be < PARKING_SPOT_LENGTH
pub const MIN_CAR_LENGTH: Distance = Distance::const_meters(4.5);
pub const MAX_CAR_LENGTH: Distance = Distance::const_meters(6.5);
// Note this is more than MAX_CAR_LENGTH
pub const BUS_LENGTH: Distance = Distance::const_meters(12.5);

// At all speeds (including at rest), cars must be at least this far apart, measured from front of
// one car to the back of the other.
pub const FOLLOWING_DISTANCE: Distance = Distance::const_meters(1.0);

// The VehicleType is only used for convenient debugging. The numeric ID itself must be sufficient.
// TODO Implement Eq, Hash, Ord manually to guarantee this.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CarID(pub usize, pub(crate) VehicleType);

impl fmt::Display for CarID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "CarID({0} -- {1})",
            self.0,
            match self.1 {
                VehicleType::Car => "car",
                VehicleType::Bus => "bus",
                VehicleType::Bike => "bike",
            }
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PedestrianID(pub usize);

impl fmt::Display for PedestrianID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "PedestrianID({0})", self.0)
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Debug, Hash)]
pub enum AgentID {
    Car(CarID),
    Pedestrian(PedestrianID),
}

impl AgentID {
    pub fn as_car(self) -> CarID {
        match self {
            AgentID::Car(id) => id,
            _ => panic!("Not a CarID: {:?}", self),
        }
    }
}

impl fmt::Display for AgentID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AgentID::Car(id) => write!(f, "AgentID({})", id),
            AgentID::Pedestrian(id) => write!(f, "AgentID({})", id),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TripID(pub usize);

impl fmt::Display for TripID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "TripID({0})", self.0)
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
pub enum VehicleType {
    Car,
    Bus,
    Bike,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Vehicle {
    pub id: CarID,
    pub owner: Option<BuildingID>,
    pub vehicle_type: VehicleType,
    pub length: Distance,
    pub max_speed: Option<Speed>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct VehicleSpec {
    pub vehicle_type: VehicleType,
    pub length: Distance,
    pub max_speed: Option<Speed>,
}

impl VehicleSpec {
    pub fn make(self, id: CarID, owner: Option<BuildingID>) -> Vehicle {
        Vehicle {
            id,
            owner,
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
}

impl ParkedCar {
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
    pub fn end_at_border(
        i: IntersectionID,
        lane_types: Vec<LaneType>,
        map: &Map,
    ) -> Option<DrivingGoal> {
        let mut lanes = Vec::new();
        for lt in lane_types {
            lanes.extend(map.get_i(i).get_incoming_lanes(map, lt));
        }
        if lanes.is_empty() {
            None
        } else {
            // TODO ideally could use any
            Some(DrivingGoal::Border(i, lanes[0]))
        }
    }

    pub fn goal_pos(&self, map: &Map) -> Position {
        let lane = match self {
            DrivingGoal::ParkNear(b) => map.find_driving_lane_near_building(*b),
            DrivingGoal::Border(_, l) => *l,
        };
        Position::new(lane, map.get_l(lane).length())
    }

    pub fn make_router(&self, path: Path, map: &Map, vt: VehicleType) -> Router {
        match self {
            DrivingGoal::ParkNear(b) => {
                if vt == VehicleType::Bike {
                    // TODO Stop closer to the building?
                    let end = path.last_step().as_lane();
                    Router::bike_then_stop(path, map.get_l(end).length() / 2.0)
                } else {
                    Router::park_near(path, *b)
                }
            }
            DrivingGoal::Border(i, last_lane) => {
                Router::end_at_border(path, map.get_l(*last_lane).length(), *i)
            }
        }
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

    // Recall sidewalks are bidirectional.
    pub fn start_at_border(i: IntersectionID, map: &Map) -> Option<SidewalkSpot> {
        let lanes = map.get_i(i).get_outgoing_lanes(map, LaneType::Sidewalk);
        if !lanes.is_empty() {
            return Some(SidewalkSpot {
                sidewalk_pos: Position::new(lanes[0], Distance::ZERO),
                connection: SidewalkPOI::Border(i),
            });
        }

        let lanes = map.get_i(i).get_incoming_lanes(map, LaneType::Sidewalk);
        if lanes.is_empty() {
            return None;
        }
        Some(SidewalkSpot {
            sidewalk_pos: Position::new(lanes[0], map.get_l(lanes[0]).length()),
            connection: SidewalkPOI::Border(i),
        })
    }

    pub fn end_at_border(i: IntersectionID, map: &Map) -> Option<SidewalkSpot> {
        let lanes = map.get_i(i).get_incoming_lanes(map, LaneType::Sidewalk);
        if !lanes.is_empty() {
            return Some(SidewalkSpot {
                sidewalk_pos: Position::new(lanes[0], map.get_l(lanes[0]).length()),
                connection: SidewalkPOI::Border(i),
            });
        }

        let lanes = map.get_i(i).get_outgoing_lanes(map, LaneType::Sidewalk);
        if lanes.is_empty() {
            return None;
        }
        Some(SidewalkSpot {
            sidewalk_pos: Position::new(lanes[0], Distance::ZERO),
            connection: SidewalkPOI::Border(i),
        })
    }

    pub fn suddenly_appear(l: LaneID, dist: Distance, map: &Map) -> SidewalkSpot {
        let lane = map.get_l(l);
        assert!(lane.is_sidewalk());
        assert!(dist <= lane.length());
        SidewalkSpot {
            sidewalk_pos: Position::new(l, dist),
            connection: SidewalkPOI::SuddenlyAppear,
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
    SuddenlyAppear,
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

    pub fn percent_clamp_end(&self, t: Duration) -> f64 {
        if t > self.end {
            return 1.0;
        }
        self.percent(t)
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
    pub speed: Speed,
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

#[derive(Serialize, Deserialize, PartialEq)]
pub struct TripPositions {
    pub time: Duration,
    pub canonical_pt_per_trip: BTreeMap<TripID, Pt2D>,
}

impl TripPositions {
    pub(crate) fn new(time: Duration) -> TripPositions {
        TripPositions {
            time,
            canonical_pt_per_trip: BTreeMap::new(),
        }
    }
}

// We have to do this in the crate where these types are defined. Bit annoying, since it's really
// kind of an ezgui concept.
impl Cloneable for ABTest {}
impl Cloneable for Scenario {}
impl Cloneable for TripID {}
impl Cloneable for TripMode {}
