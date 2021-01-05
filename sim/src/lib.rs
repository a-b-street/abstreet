//! The sim crate runs a traffic simulation on top of the map_model. See also
//! https://dabreegster.github.io/abstreet/trafficsim/index.html.
//!
//! The simulation is very roughly layered into two pieces: the low-level "mechanics" of simulating
//! individual agents over time, and higher-level systems like TripManager and TransitSimState that
//! glue together individual goals executed by the agents.
//!
//! Helpful terminology:
//! - sov = single occupancy vehicle, a car with just a driver and no passengers. (Car passengers
//!   are not currently modelled)

#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate log;

use std::fmt;

use serde::{Deserialize, Serialize};

use abstutil::{deserialize_usize, serialize_usize};
use geom::{Distance, Speed, Time};
use map_model::{
    BuildingID, BusRouteID, BusStopID, IntersectionID, LaneID, Map, ParkingLotID, Path,
    PathConstraints, Position,
};

pub use crate::render::{
    CarStatus, DrawCarInput, DrawPedCrowdInput, DrawPedestrianInput, PedCrowdLocation,
    UnzoomedAgent,
};

pub use self::analytics::{Analytics, TripPhase};
pub(crate) use self::cap::CapSimState;
pub(crate) use self::events::Event;
pub use self::events::{AlertLocation, TripPhaseType};
pub(crate) use self::make::TripSpec;
pub use self::make::{
    fork_rng, BorderSpawnOverTime, ExternalPerson, ExternalTrip, ExternalTripEndpoint, IndividTrip,
    PersonSpec, Scenario, ScenarioGenerator, ScenarioModifier, SimFlags, SpawnOverTime,
    TripEndpoint, TripPurpose,
};
pub(crate) use self::mechanics::{
    DrivingSimState, IntersectionSimState, ParkingSim, ParkingSimState, WalkingSimState,
};
pub(crate) use self::pandemic::PandemicModel;
pub(crate) use self::recorder::TrafficRecorder;
pub(crate) use self::router::{ActionAtEnd, Router};
pub(crate) use self::scheduler::{Command, Scheduler};
pub use self::sim::{AgentProperties, AlertHandler, DelayCause, Sim, SimCallback, SimOptions};
pub(crate) use self::transit::TransitSimState;
pub use self::trips::TripMode;
pub use self::trips::{CommutersVehiclesCounts, Person, PersonState, TripInfo, TripResult};
pub(crate) use self::trips::{TripLeg, TripManager};

mod analytics;
mod cap;
mod events;
mod make;
mod mechanics;
mod pandemic;
mod recorder;
mod render;
mod router;
mod scheduler;
mod sim;
mod transit;
mod trips;

// http://pccsc.net/bicycle-parking-info/ says 68 inches, which is 1.73m
pub(crate) const BIKE_LENGTH: Distance = Distance::const_meters(1.8);
// These two must be < PARKING_SPOT_LENGTH
pub(crate) const MIN_CAR_LENGTH: Distance = Distance::const_meters(4.5);
pub(crate) const MAX_CAR_LENGTH: Distance = Distance::const_meters(6.5);
// Note this is more than MAX_CAR_LENGTH
pub(crate) const BUS_LENGTH: Distance = Distance::const_meters(12.5);
pub(crate) const LIGHT_RAIL_LENGTH: Distance = Distance::const_meters(60.0);

/// At all speeds (including at rest), cars must be at least this far apart, measured from front of
/// one car to the back of the other.
pub(crate) const FOLLOWING_DISTANCE: Distance = Distance::const_meters(1.0);

/// When spawning at borders, start the front of the vehicle this far along and gradually appear.
/// Getting too close to EPSILON_DIST can lead to get_draw_car having no geometry at all.
pub(crate) const SPAWN_DIST: Distance = Distance::const_meters(0.05);

/// The numeric ID must be globally unique, without considering VehicleType. VehicleType is bundled
/// for convenient debugging.
// TODO Implement Eq, Hash, Ord manually to guarantee this.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CarID(
    #[serde(
        serialize_with = "serialize_usize",
        deserialize_with = "deserialize_usize"
    )]
    pub usize,
    pub VehicleType,
);

impl fmt::Display for CarID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.1 {
            VehicleType::Car => write!(f, "Car #{}", self.0),
            VehicleType::Bus => write!(f, "Bus #{}", self.0),
            VehicleType::Train => write!(f, "Train #{}", self.0),
            VehicleType::Bike => write!(f, "Bike #{}", self.0),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PedestrianID(
    #[serde(
        serialize_with = "serialize_usize",
        deserialize_with = "deserialize_usize"
    )]
    pub usize,
);

impl fmt::Display for PedestrianID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Pedestrian #{}", self.0)
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Debug, Hash)]
pub enum AgentID {
    Car(CarID),
    Pedestrian(PedestrianID),
    // TODO Rename...
    BusPassenger(PersonID, CarID),
}

impl AgentID {
    pub(crate) fn as_car(self) -> CarID {
        match self {
            AgentID::Car(id) => id,
            _ => panic!("Not a CarID: {:?}", self),
        }
    }

    pub fn to_type(self) -> AgentType {
        match self {
            AgentID::Car(c) => match c.1 {
                VehicleType::Car => AgentType::Car,
                VehicleType::Bike => AgentType::Bike,
                VehicleType::Bus => AgentType::Bus,
                VehicleType::Train => AgentType::Train,
            },
            AgentID::Pedestrian(_) => AgentType::Pedestrian,
            AgentID::BusPassenger(_, _) => AgentType::TransitRider,
        }
    }

    pub fn to_vehicle_type(self) -> Option<VehicleType> {
        match self {
            AgentID::Car(c) => Some(c.1),
            AgentID::Pedestrian(_) => None,
            AgentID::BusPassenger(_, _) => None,
        }
    }
}

impl fmt::Display for AgentID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AgentID::Car(id) => write!(f, "AgentID({})", id),
            AgentID::Pedestrian(id) => write!(f, "AgentID({})", id),
            AgentID::BusPassenger(person, bus) => write!(f, "AgentID({} on {})", person, bus),
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Debug, Hash)]
pub enum AgentType {
    Car,
    Bike,
    Bus,
    Train,
    Pedestrian,
    TransitRider,
}

impl AgentType {
    pub fn all() -> Vec<AgentType> {
        vec![
            AgentType::Car,
            AgentType::Bike,
            AgentType::Bus,
            AgentType::Train,
            AgentType::Pedestrian,
            AgentType::TransitRider,
        ]
    }

    pub fn noun(self) -> &'static str {
        match self {
            AgentType::Car => "Car",
            AgentType::Bike => "Bike",
            AgentType::Bus => "Bus",
            AgentType::Train => "Train",
            AgentType::Pedestrian => "Pedestrian",
            AgentType::TransitRider => "Transit rider",
        }
    }

    pub fn plural_noun(self) -> &'static str {
        match self {
            AgentType::Car => "cars",
            AgentType::Bike => "bikes",
            AgentType::Bus => "buses",
            AgentType::Train => "trains",
            AgentType::Pedestrian => "pedestrians",
            AgentType::TransitRider => "transit riders",
        }
    }

    pub fn ongoing_verb(self) -> &'static str {
        match self {
            AgentType::Car => "driving",
            AgentType::Bike => "biking",
            AgentType::Bus | AgentType::Train => unreachable!(),
            AgentType::Pedestrian => "walking",
            AgentType::TransitRider => "riding transit",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TripID(
    #[serde(
        serialize_with = "serialize_usize",
        deserialize_with = "deserialize_usize"
    )]
    pub usize,
);

impl fmt::Display for TripID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Trip #{}", self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PersonID(
    #[serde(
        serialize_with = "serialize_usize",
        deserialize_with = "deserialize_usize"
    )]
    pub usize,
);

impl fmt::Display for PersonID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Person {}", self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct OrigPersonID(
    #[serde(
        serialize_with = "serialize_usize",
        deserialize_with = "deserialize_usize"
    )]
    pub usize,
    #[serde(
        serialize_with = "serialize_usize",
        deserialize_with = "deserialize_usize"
    )]
    pub usize,
);

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
pub enum VehicleType {
    Car,
    Bus,
    Train,
    Bike,
}

impl fmt::Display for VehicleType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            VehicleType::Car => write!(f, "car"),
            VehicleType::Bus => write!(f, "bus"),
            VehicleType::Train => write!(f, "train"),
            VehicleType::Bike => write!(f, "bike"),
        }
    }
}

impl VehicleType {
    pub fn to_constraints(self) -> PathConstraints {
        match self {
            VehicleType::Car => PathConstraints::Car,
            VehicleType::Bus => PathConstraints::Bus,
            VehicleType::Train => PathConstraints::Train,
            VehicleType::Bike => PathConstraints::Bike,
        }
    }

    pub(crate) fn is_transit(self) -> bool {
        match self {
            VehicleType::Car => false,
            VehicleType::Bus => true,
            VehicleType::Train => true,
            VehicleType::Bike => false,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Vehicle {
    pub id: CarID,
    pub owner: Option<PersonID>,
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
    pub(crate) fn make(self, id: CarID, owner: Option<PersonID>) -> Vehicle {
        assert_eq!(id.1, self.vehicle_type);
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
pub enum ParkingSpot {
    /// Lane and idx
    Onstreet(LaneID, usize),
    /// Building and idx (pretty meaningless)
    Offstreet(BuildingID, usize),
    Lot(ParkingLotID, usize),
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct ParkedCar {
    pub vehicle: Vehicle,
    pub spot: ParkingSpot,
    pub parked_since: Time,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum DrivingGoal {
    ParkNear(BuildingID),
    Border(IntersectionID, LaneID),
}

impl DrivingGoal {
    pub fn goal_pos(&self, constraints: PathConstraints, map: &Map) -> Option<Position> {
        match self {
            DrivingGoal::ParkNear(b) => match constraints {
                PathConstraints::Car => {
                    let driving_lane = map.find_driving_lane_near_building(*b);
                    let sidewalk_pos = map.get_b(*b).sidewalk_pos;
                    if map.get_l(driving_lane).parent == map.get_l(sidewalk_pos.lane()).parent {
                        Some(sidewalk_pos.equiv_pos(driving_lane, map))
                    } else {
                        Some(Position::start(driving_lane))
                    }
                }
                PathConstraints::Bike => Some(map.get_b(*b).biking_connection(map)?.0),
                PathConstraints::Bus | PathConstraints::Train | PathConstraints::Pedestrian => {
                    unreachable!()
                }
            },
            DrivingGoal::Border(_, l) => Some(Position::end(*l, map)),
        }
    }

    pub fn make_router(&self, owner: CarID, path: Path, map: &Map) -> Router {
        match self {
            DrivingGoal::ParkNear(b) => {
                if owner.1 == VehicleType::Bike {
                    Router::bike_then_stop(owner, path, SidewalkSpot::bike_rack(*b, map).unwrap())
                } else {
                    Router::park_near(owner, path, *b)
                }
            }
            DrivingGoal::Border(i, last_lane) => {
                Router::end_at_border(owner, path, map.get_l(*last_lane).length(), *i)
            }
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub(crate) struct SidewalkSpot {
    pub connection: SidewalkPOI,
    pub sidewalk_pos: Position,
}

/// Point of interest, that is
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) enum SidewalkPOI {
    /// Note that for offstreet parking, the path will be the same as the building's front path.
    ParkingSpot(ParkingSpot),
    /// Don't actually know where this goes yet!
    DeferredParkingSpot,
    Building(BuildingID),
    BusStop(BusStopID),
    Border(IntersectionID),
    /// The bikeable position
    BikeRack(Position),
    SuddenlyAppear,
}

impl SidewalkSpot {
    /// Pretty hacky case
    pub fn deferred_parking_spot() -> SidewalkSpot {
        SidewalkSpot {
            connection: SidewalkPOI::DeferredParkingSpot,
            // Dummy value
            sidewalk_pos: Position::start(LaneID(0)),
        }
    }

    pub fn parking_spot(
        spot: ParkingSpot,
        map: &Map,
        parking_sim: &ParkingSimState,
    ) -> SidewalkSpot {
        SidewalkSpot {
            connection: SidewalkPOI::ParkingSpot(spot),
            sidewalk_pos: parking_sim.spot_to_sidewalk_pos(spot, map),
        }
    }

    pub fn building(b: BuildingID, map: &Map) -> SidewalkSpot {
        SidewalkSpot {
            connection: SidewalkPOI::Building(b),
            sidewalk_pos: map.get_b(b).sidewalk_pos,
        }
    }

    // TODO For the case when we have to start/stop biking somewhere else, this won't match up with
    // a building though!
    pub fn bike_rack(b: BuildingID, map: &Map) -> Option<SidewalkSpot> {
        let (bike_pos, sidewalk_pos) = map.get_b(b).biking_connection(map)?;
        Some(SidewalkSpot {
            connection: SidewalkPOI::BikeRack(bike_pos),
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
        let lanes = map
            .get_i(i)
            .get_outgoing_lanes(map, PathConstraints::Pedestrian);
        if !lanes.is_empty() {
            return Some(SidewalkSpot {
                sidewalk_pos: Position::start(lanes[0]),
                connection: SidewalkPOI::Border(i),
            });
        }

        map.get_i(i)
            .get_incoming_lanes(map, PathConstraints::Pedestrian)
            .get(0)
            .map(|l| SidewalkSpot {
                sidewalk_pos: Position::end(*l, map),
                connection: SidewalkPOI::Border(i),
            })
    }

    pub fn end_at_border(i: IntersectionID, map: &Map) -> Option<SidewalkSpot> {
        if let Some(l) = map
            .get_i(i)
            .get_incoming_lanes(map, PathConstraints::Pedestrian)
            .get(0)
        {
            return Some(SidewalkSpot {
                sidewalk_pos: Position::end(*l, map),
                connection: SidewalkPOI::Border(i),
            });
        }

        let lanes = map
            .get_i(i)
            .get_outgoing_lanes(map, PathConstraints::Pedestrian);
        if lanes.is_empty() {
            return None;
        }
        Some(SidewalkSpot {
            sidewalk_pos: Position::start(lanes[0]),
            connection: SidewalkPOI::Border(i),
        })
    }

    pub fn suddenly_appear(pos: Position, map: &Map) -> SidewalkSpot {
        let lane = map.get_l(pos.lane());
        assert!(lane.is_walkable());
        assert!(pos.dist_along() <= lane.length());
        SidewalkSpot {
            sidewalk_pos: pos,
            connection: SidewalkPOI::SuddenlyAppear,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, Copy)]
pub(crate) struct TimeInterval {
    // TODO Private fields
    pub start: Time,
    pub end: Time,
}

impl TimeInterval {
    pub fn new(start: Time, end: Time) -> TimeInterval {
        if end < start {
            panic!("Bad TimeInterval {} .. {}", start, end);
        }
        TimeInterval { start, end }
    }

    pub fn percent(&self, t: Time) -> f64 {
        if self.start == self.end {
            return 1.0;
        }

        let x = (t - self.start) / (self.end - self.start);
        assert!(x >= 0.0 && x <= 1.0);
        x
    }

    pub fn percent_clamp_end(&self, t: Time) -> f64 {
        if t > self.end {
            return 1.0;
        }
        self.percent(t)
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, Copy)]
pub(crate) struct DistanceInterval {
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

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub(crate) struct CreatePedestrian {
    pub id: PedestrianID,
    pub start: SidewalkSpot,
    pub speed: Speed,
    pub goal: SidewalkSpot,
    pub path: Path,
    pub trip: TripID,
    pub person: PersonID,
}

#[derive(Serialize, Deserialize, PartialEq, Clone, Debug)]
pub(crate) struct CreateCar {
    pub vehicle: Vehicle,
    pub router: Router,
    pub maybe_parked_car: Option<ParkedCar>,
    /// None for buses
    pub trip_and_person: Option<(TripID, PersonID)>,
    pub maybe_route: Option<BusRouteID>,
}

impl CreateCar {
    pub fn for_appearing(
        vehicle: Vehicle,
        router: Router,
        trip: TripID,
        person: PersonID,
    ) -> CreateCar {
        CreateCar {
            vehicle,
            router,
            maybe_parked_car: None,
            trip_and_person: Some((trip, person)),
            maybe_route: None,
        }
    }

    // TODO Maybe inline in trips, the only caller.
    pub fn for_parked_car(
        parked_car: ParkedCar,
        router: Router,
        trip: TripID,
        person: PersonID,
    ) -> CreateCar {
        CreateCar {
            vehicle: parked_car.vehicle.clone(),
            router,
            maybe_parked_car: Some(parked_car),
            trip_and_person: Some((trip, person)),
            maybe_route: None,
        }
    }
}
