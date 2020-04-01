mod analytics;
mod events;
mod make;
mod mechanics;
mod pandemic;
mod render;
mod router;
mod scheduler;
mod sim;
mod transit;
mod trips;

pub use self::analytics::{Analytics, TripPhase};
pub use self::events::{Event, TripPhaseType};
pub use self::make::{
    ABTest, BorderSpawnOverTime, IndividTrip, OriginDestination, PersonSpec, Population, Scenario,
    SeedParkedCars, SimFlags, SpawnOverTime, SpawnTrip, TripSpawner, TripSpec,
};
pub(crate) use self::mechanics::{
    DrivingSimState, IntersectionSimState, ParkingSimState, WalkingSimState,
};
pub(crate) use self::router::{ActionAtEnd, Router};
pub(crate) use self::scheduler::{Command, Scheduler};
pub use self::sim::{Sim, SimOptions};
pub(crate) use self::transit::TransitSimState;
pub use self::trips::{Person, PersonState, TripCount, TripResult};
pub use self::trips::{TripEndpoint, TripMode};
pub(crate) use self::trips::{TripLeg, TripManager};
pub use crate::render::{
    CarStatus, DontDrawAgents, DrawCarInput, DrawPedCrowdInput, DrawPedestrianInput, GetDrawAgents,
    PedCrowdLocation, UnzoomedAgent,
};
use abstutil::Cloneable;
use geom::{Distance, Pt2D, Speed, Time};
use map_model::{
    BuildingID, BusStopID, DirectedRoadID, IntersectionID, LaneID, Map, Path, PathConstraints,
    PathRequest, Position,
};
pub use pandemic::PandemicModel;
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

// The numeric ID must be globally unique, without considering VehicleType. VehicleType is bundled
// for convenient debugging.
// TODO Implement Eq, Hash, Ord manually to guarantee this.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CarID(pub usize, pub VehicleType);

impl fmt::Display for CarID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.1 {
            VehicleType::Car => write!(f, "Car #{}", self.0),
            VehicleType::Bus => write!(f, "Bus #{}", self.0),
            VehicleType::Bike => write!(f, "Bike #{}", self.0),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PedestrianID(pub usize);

impl fmt::Display for PedestrianID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Pedestrian #{}", self.0)
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
        write!(f, "Trip #{}", self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PersonID(pub usize);

impl fmt::Display for PersonID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Person {}", self.0)
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
pub enum VehicleType {
    Car,
    Bus,
    Bike,
}

impl fmt::Display for VehicleType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            VehicleType::Car => write!(f, "car"),
            VehicleType::Bus => write!(f, "bus"),
            VehicleType::Bike => write!(f, "bike"),
        }
    }
}

impl VehicleType {
    pub fn to_constraints(self) -> PathConstraints {
        match self {
            VehicleType::Car => PathConstraints::Car,
            VehicleType::Bus => PathConstraints::Bus,
            VehicleType::Bike => PathConstraints::Bike,
        }
    }
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
pub enum ParkingSpot {
    // Lane and idx
    Onstreet(LaneID, usize),
    // Building and idx (pretty meaningless)
    Offstreet(BuildingID, usize),
}

impl ParkingSpot {
    pub fn onstreet(lane: LaneID, idx: usize) -> ParkingSpot {
        ParkingSpot::Onstreet(lane, idx)
    }

    pub fn offstreet(bldg: BuildingID, idx: usize) -> ParkingSpot {
        ParkingSpot::Offstreet(bldg, idx)
    }
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct ParkedCar {
    pub vehicle: Vehicle,
    pub spot: ParkingSpot,
}

// It'd be nice to inline the goal_pos like SidewalkSpot does, but DrivingGoal is persisted in
// Scenarios, so this wouldn't survive map edits.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DrivingGoal {
    ParkNear(BuildingID),
    Border(IntersectionID, LaneID),
}

impl DrivingGoal {
    pub fn end_at_border(
        dr: DirectedRoadID,
        constraints: PathConstraints,
        map: &Map,
    ) -> Option<DrivingGoal> {
        let lanes = dr.lanes(constraints, map);
        if lanes.is_empty() {
            None
        } else {
            // TODO ideally could use any
            Some(DrivingGoal::Border(dr.dst_i(map), lanes[0]))
        }
    }

    pub fn goal_pos(&self, constraints: PathConstraints, map: &Map) -> Position {
        match self {
            DrivingGoal::ParkNear(b) => match constraints {
                PathConstraints::Car => {
                    Position::new(map.find_driving_lane_near_building(*b), Distance::ZERO)
                }
                PathConstraints::Bike => {
                    let l = map.find_biking_lane_near_building(*b);
                    Position::new(l, map.get_l(l).length() / 2.0)
                }
                PathConstraints::Bus | PathConstraints::Pedestrian => unreachable!(),
            },
            DrivingGoal::Border(_, l) => Position::new(*l, map.get_l(*l).length()),
        }
    }

    pub(crate) fn make_router(&self, path: Path, map: &Map, vt: VehicleType) -> Router {
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

    pub fn pt(&self, map: &Map) -> Pt2D {
        match self {
            DrivingGoal::ParkNear(b) => map.get_b(*b).polygon.center(),
            DrivingGoal::Border(i, _) => map.get_i(*i).polygon.center(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct SidewalkSpot {
    pub connection: SidewalkPOI,
    pub sidewalk_pos: Position,
}

impl SidewalkSpot {
    // Pretty hacky case
    pub fn deferred_parking_spot(
        start_bldg: BuildingID,
        goal: DrivingGoal,
        map: &Map,
    ) -> SidewalkSpot {
        SidewalkSpot {
            connection: SidewalkPOI::DeferredParkingSpot(start_bldg, goal),
            // Dummy value
            sidewalk_pos: map.get_b(start_bldg).front_path.sidewalk,
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
        let driving_pos = sidewalk_pos.equiv_pos(driving_lane, Distance::ZERO, map);
        Some(SidewalkSpot {
            connection: SidewalkPOI::BikeRack(driving_pos),
            sidewalk_pos,
        })
    }

    pub fn bike_from_bike_rack(sidewalk: LaneID, map: &Map) -> Option<SidewalkSpot> {
        assert!(map.get_l(sidewalk).is_sidewalk());
        let driving_lane = map.get_parent(sidewalk).sidewalk_to_bike(sidewalk)?;
        // Don't start biking on a blackhole!
        // TODO Maybe compute a separate blackhole graph that includes bike lanes.
        if let Some(redirect) = map.get_l(driving_lane).parking_blackhole {
            let new_sidewalk = map.get_parent(redirect).bike_to_sidewalk(redirect)?;
            SidewalkSpot::bike_rack(new_sidewalk, map)
        } else {
            SidewalkSpot::bike_rack(sidewalk, map)
        }
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
                sidewalk_pos: Position::new(lanes[0], Distance::ZERO),
                connection: SidewalkPOI::Border(i),
            });
        }

        let lanes = map
            .get_i(i)
            .get_incoming_lanes(map, PathConstraints::Pedestrian);
        if lanes.is_empty() {
            return None;
        }
        Some(SidewalkSpot {
            sidewalk_pos: Position::new(lanes[0], map.get_l(lanes[0]).length()),
            connection: SidewalkPOI::Border(i),
        })
    }

    pub fn end_at_border(i: IntersectionID, map: &Map) -> Option<SidewalkSpot> {
        let lanes = map
            .get_i(i)
            .get_incoming_lanes(map, PathConstraints::Pedestrian);
        if !lanes.is_empty() {
            return Some(SidewalkSpot {
                sidewalk_pos: Position::new(lanes[0], map.get_l(lanes[0]).length()),
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
    // Note that for offstreet parking, the path will be the same as the building's front path.
    ParkingSpot(ParkingSpot),
    // Don't actually know where this goes yet!
    DeferredParkingSpot(BuildingID, DrivingGoal),
    Building(BuildingID),
    BusStop(BusStopID),
    Border(IntersectionID),
    // The equivalent position on the nearest driving/bike lane
    BikeRack(Position),
    SuddenlyAppear,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct TimeInterval {
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

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
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

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct CreatePedestrian {
    pub id: PedestrianID,
    pub start: SidewalkSpot,
    pub speed: Speed,
    pub goal: SidewalkSpot,
    pub req: PathRequest,
    pub path: Path,
    pub trip: TripID,
    pub person: PersonID,
}

#[derive(Serialize, Deserialize, PartialEq, Clone, Debug)]
pub struct CreateCar {
    pub vehicle: Vehicle,
    pub router: Router,
    pub req: PathRequest,
    pub start_dist: Distance,
    pub maybe_parked_car: Option<ParkedCar>,
    // None for buses
    pub trip: Option<TripID>,
    pub person: Option<PersonID>,
}

impl CreateCar {
    pub fn for_appearing(
        vehicle: Vehicle,
        start_pos: Position,
        router: Router,
        req: PathRequest,
        trip: TripID,
        person: PersonID,
    ) -> CreateCar {
        CreateCar {
            vehicle,
            router,
            req,
            start_dist: start_pos.dist_along(),
            maybe_parked_car: None,
            trip: Some(trip),
            person: Some(person),
        }
    }

    // TODO Maybe inline in trips, the only caller.
    pub fn for_parked_car(
        parked_car: ParkedCar,
        router: Router,
        req: PathRequest,
        start_dist: Distance,
        trip: TripID,
        person: PersonID,
    ) -> CreateCar {
        CreateCar {
            vehicle: parked_car.vehicle.clone(),
            router,
            req,
            start_dist,
            maybe_parked_car: Some(parked_car),
            trip: Some(trip),
            person: Some(person),
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq, Clone)]
pub struct TripPositions {
    pub time: Time,
    pub canonical_pt_per_trip: BTreeMap<TripID, Pt2D>,
}

impl TripPositions {
    pub(crate) fn new(time: Time) -> TripPositions {
        TripPositions {
            time,
            canonical_pt_per_trip: BTreeMap::new(),
        }
    }
}

// We have to do this in the crate where these types are defined. Bit annoying, since it's really
// kind of an ezgui concept.
impl Cloneable for ABTest {}
impl Cloneable for CarID {}
impl Cloneable for Scenario {}
impl Cloneable for TripID {}
impl Cloneable for TripMode {}
