use geom::{Angle, Pt2D};
use map_model::{LaneID, LaneType, Map, Trace, TurnID};
use {CarID, Distance, PedestrianID, Sim};

// Intermediate structures so that sim and editor crates don't have a cyclic dependency.
pub struct DrawPedestrianInput {
    pub id: PedestrianID,
    pub pos: Pt2D,
    pub waiting_for_turn: Option<TurnID>,
    pub preparing_bike: bool,
}

pub struct DrawCarInput {
    pub id: CarID,
    pub vehicle_length: Distance,
    pub waiting_for_turn: Option<TurnID>,
    pub front: Pt2D,
    pub angle: Angle,
    pub stopping_trace: Option<Trace>,
    pub state: CarState,
}

#[derive(PartialEq, Eq)]
pub enum CarState {
    Moving,
    Stuck,
    Parked,
    Debug,
}

impl Sim {
    pub fn get_draw_car(&self, id: CarID, map: &Map) -> Option<DrawCarInput> {
        self.driving_state
            .get_draw_car(id, self.time, map)
            .or_else(|| self.parking_state.get_draw_car(id))
    }

    pub fn get_draw_ped(&self, id: PedestrianID, map: &Map) -> Option<DrawPedestrianInput> {
        self.walking_state.get_draw_ped(id, map)
    }

    // TODO maybe just DrawAgent instead? should caller care?
    pub fn get_draw_cars_on_lane(&self, l: LaneID, map: &Map) -> Vec<DrawCarInput> {
        match map.get_l(l).lane_type {
            LaneType::Driving | LaneType::Bus | LaneType::Biking => {
                self.driving_state.get_draw_cars_on_lane(l, self.time, map)
            }
            LaneType::Parking => self.parking_state.get_draw_cars(l),
            LaneType::Sidewalk => Vec::new(),
        }
    }

    pub fn get_draw_cars_on_turn(&self, t: TurnID, map: &Map) -> Vec<DrawCarInput> {
        self.driving_state.get_draw_cars_on_turn(t, self.time, map)
    }

    pub fn get_draw_peds_on_lane(&self, l: LaneID, map: &Map) -> Vec<DrawPedestrianInput> {
        self.walking_state.get_draw_peds_on_lane(l, map)
    }

    pub fn get_draw_peds_on_turn(&self, t: TurnID, map: &Map) -> Vec<DrawPedestrianInput> {
        self.walking_state.get_draw_peds_on_turn(t, map)
    }
}
