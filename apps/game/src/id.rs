use map_model::{AreaID, BuildingID, IntersectionID, LaneID, ParkingLotID, RoadID, TransitStopID};
use sim::{AgentID, CarID, PedestrianID};

#[derive(Clone, Hash, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub enum ID {
    Road(RoadID),
    Lane(LaneID),
    Intersection(IntersectionID),
    Building(BuildingID),
    ParkingLot(ParkingLotID),
    Car(CarID),
    Pedestrian(PedestrianID),
    PedCrowd(Vec<PedestrianID>),
    TransitStop(TransitStopID),
    Area(AreaID),
}

impl ID {
    pub fn from_agent(id: AgentID) -> ID {
        match id {
            AgentID::Car(id) => ID::Car(id),
            AgentID::Pedestrian(id) => ID::Pedestrian(id),
            AgentID::BusPassenger(_, bus) => ID::Car(bus),
        }
    }

    pub fn agent_id(&self) -> Option<AgentID> {
        match *self {
            ID::Car(id) => Some(AgentID::Car(id)),
            ID::Pedestrian(id) => Some(AgentID::Pedestrian(id)),
            // PedCrowd doesn't map to a single agent.
            _ => None,
        }
    }

    pub fn as_intersection(&self) -> IntersectionID {
        match *self {
            ID::Intersection(i) => i,
            _ => panic!("Can't call as_intersection on {:?}", self),
        }
    }

    pub fn to_map_gui(self) -> map_gui::ID {
        match self {
            Self::Road(x) => map_gui::ID::Road(x),
            Self::Lane(x) => map_gui::ID::Lane(x),
            Self::Intersection(x) => map_gui::ID::Intersection(x),
            Self::Building(x) => map_gui::ID::Building(x),
            Self::ParkingLot(x) => map_gui::ID::ParkingLot(x),
            Self::TransitStop(x) => map_gui::ID::TransitStop(x),
            Self::Area(x) => map_gui::ID::Area(x),
            _ => panic!("Can't call map_gui on {:?}", self),
        }
    }
}

impl From<map_gui::ID> for ID {
    fn from(id: map_gui::ID) -> Self {
        match id {
            map_gui::ID::Road(x) => Self::Road(x),
            map_gui::ID::Lane(x) => Self::Lane(x),
            map_gui::ID::Intersection(x) => Self::Intersection(x),
            map_gui::ID::Building(x) => Self::Building(x),
            map_gui::ID::ParkingLot(x) => Self::ParkingLot(x),
            map_gui::ID::TransitStop(x) => Self::TransitStop(x),
            map_gui::ID::Area(x) => Self::Area(x),
        }
    }
}
