use geom::{Angle, Pt2D};
use map_model::{LaneID, LaneType, Map, Trace, TurnID};
use {CarID, Distance, PedestrianID, Sim, VehicleType};

// Intermediate structures so that sim and editor crates don't have a cyclic dependency.
#[derive(Clone)]
pub struct DrawPedestrianInput {
    pub id: PedestrianID,
    pub pos: Pt2D,
    pub waiting_for_turn: Option<TurnID>,
    pub preparing_bike: bool,
}

#[derive(Clone)]
pub struct DrawCarInput {
    pub id: CarID,
    pub vehicle_length: Distance,
    pub waiting_for_turn: Option<TurnID>,
    pub front: Pt2D,
    pub angle: Angle,
    pub stopping_trace: Option<Trace>,
    pub state: CarState,
    pub vehicle_type: VehicleType,
}

#[derive(Clone, PartialEq, Eq)]
pub enum CarState {
    Moving,
    Stuck,
    Parked,
    Debug,
}

// TODO Can we return borrows instead? Nice for time travel, not for main sim?
// actually good for main sim too; we're constantly calculating stuff while sim is paused
// otherwise? except we don't know what to calculate. maybe cache it?
pub trait GetDrawAgents {
    fn get_draw_car(&self, id: CarID, map: &Map) -> Option<DrawCarInput>;
    fn get_draw_ped(&self, id: PedestrianID, map: &Map) -> Option<DrawPedestrianInput>;
    fn get_draw_cars_on_lane(&self, l: LaneID, map: &Map) -> Vec<DrawCarInput>;
    fn get_draw_cars_on_turn(&self, t: TurnID, map: &Map) -> Vec<DrawCarInput>;
    fn get_draw_peds_on_lane(&self, l: LaneID, map: &Map) -> Vec<DrawPedestrianInput>;
    fn get_draw_peds_on_turn(&self, t: TurnID, map: &Map) -> Vec<DrawPedestrianInput>;
}

impl GetDrawAgents for Sim {
    fn get_draw_car(&self, id: CarID, map: &Map) -> Option<DrawCarInput> {
        self.driving_state
            .get_draw_car(id, self.time, map)
            .or_else(|| self.parking_state.get_draw_car(id))
    }

    fn get_draw_ped(&self, id: PedestrianID, map: &Map) -> Option<DrawPedestrianInput> {
        self.walking_state.get_draw_ped(id, map, self.time)
    }

    // TODO maybe just DrawAgent instead? should caller care?
    fn get_draw_cars_on_lane(&self, l: LaneID, map: &Map) -> Vec<DrawCarInput> {
        match map.get_l(l).lane_type {
            LaneType::Driving | LaneType::Bus | LaneType::Biking => {
                self.driving_state.get_draw_cars_on_lane(l, self.time, map)
            }
            LaneType::Parking => self.parking_state.get_draw_cars(l),
            LaneType::Sidewalk => Vec::new(),
        }
    }

    fn get_draw_cars_on_turn(&self, t: TurnID, map: &Map) -> Vec<DrawCarInput> {
        self.driving_state.get_draw_cars_on_turn(t, self.time, map)
    }

    fn get_draw_peds_on_lane(&self, l: LaneID, map: &Map) -> Vec<DrawPedestrianInput> {
        self.walking_state.get_draw_peds_on_lane(l, map, self.time)
    }

    fn get_draw_peds_on_turn(&self, t: TurnID, map: &Map) -> Vec<DrawPedestrianInput> {
        self.walking_state.get_draw_peds_on_turn(t, map, self.time)
    }
}
