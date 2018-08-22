use dimensioned::si;
use draw_car::DrawCar;
use geom::{Angle, Pt2D};
use kinematics::Vehicle;
use map_model;
use map_model::{Lane, LaneID, LaneType, Map};
use sim::CarParking;
use std::collections::BTreeMap;
use std::iter;
use {CarID, Distance};

#[derive(Serialize, Deserialize, PartialEq, Eq)]
pub struct ParkingSimState {
    // TODO hacky, but other types of lanes just mark 0 spots. :\
    lanes: Vec<ParkingLane>,
    total_count: usize,
}

impl ParkingSimState {
    pub fn new(map: &Map) -> ParkingSimState {
        ParkingSimState {
            lanes: map.all_lanes()
                .iter()
                .map(|l| ParkingLane::new(l))
                .collect(),
            total_count: 0,
        }
    }

    pub fn edit_remove_lane(&mut self, id: LaneID) {
        assert!(self.lanes[id.0].is_empty());
        self.lanes[id.0] = ParkingLane {
            id: id,
            spots: Vec::new(),
            occupants: Vec::new(),
        };
    }

    pub fn edit_add_lane(&mut self, l: &Lane) {
        self.lanes[l.id.0] = ParkingLane::new(l);
    }

    pub fn total_count(&self) -> usize {
        self.total_count
    }

    pub fn get_all_spots(&self, lane: LaneID) -> Vec<ParkingSpot> {
        self.lanes[lane.0].spots.clone()
    }

    pub fn get_all_free_spots(&self) -> Vec<ParkingSpot> {
        let mut spots: Vec<ParkingSpot> = Vec::new();
        for l in &self.lanes {
            for (spot, occupant) in l.spots.iter().zip(l.occupants.iter()) {
                if occupant.is_none() {
                    spots.push(spot.clone());
                }
            }
        }
        spots
    }

    pub fn remove_parked_car(&mut self, id: LaneID, car: CarID) {
        self.lanes[id.0].remove_parked_car(car);
        self.total_count -= 1;
    }

    pub fn add_parked_car(&mut self, p: CarParking) {
        assert_eq!(
            self.lanes[p.spot.parking_lane.0].occupants[p.spot.spot_idx],
            None
        );
        self.lanes[p.spot.parking_lane.0].occupants[p.spot.spot_idx] = Some(p.car);
        self.total_count += 1;
    }

    pub fn get_draw_cars(
        &self,
        id: LaneID,
        map: &Map,
        properties: &BTreeMap<CarID, Vehicle>,
    ) -> Vec<DrawCar> {
        self.lanes[id.0].get_draw_cars(map, properties)
    }

    pub fn get_draw_car(
        &self,
        id: CarID,
        map: &Map,
        properties: &BTreeMap<CarID, Vehicle>,
    ) -> Option<DrawCar> {
        // TODO this is so horrendously slow :D
        for l in &self.lanes {
            if l.occupants.contains(&Some(id)) {
                return l.get_draw_cars(map, properties)
                    .into_iter()
                    .find(|c| c.id == id);
            }
        }
        None
    }

    pub fn lane_of_car(&self, id: CarID) -> Option<LaneID> {
        // TODO this is so horrendously slow :D
        for l in &self.lanes {
            if l.occupants.contains(&Some(id)) {
                return Some(l.id);
            }
        }
        None
    }

    // Of the front of the car
    pub fn get_spot_of_car(&self, c: CarID, l: LaneID) -> ParkingSpot {
        let idx = self.lanes[l.0]
            .occupants
            .iter()
            .position(|x| *x == Some(c))
            .unwrap();
        self.lanes[l.0].spots[idx].clone()
    }

    pub fn get_all_cars(&self) -> Vec<(CarID, LaneID)> {
        let mut result = Vec::new();
        for l in &self.lanes {
            for maybe_car in &l.occupants {
                if let Some(car) = maybe_car {
                    result.push((*car, l.id));
                }
            }
        }
        result
    }

    pub fn get_first_free_spot(&self, lane: LaneID, dist_along: Distance) -> Option<ParkingSpot> {
        let l = &self.lanes[lane.0];
        // Just require the car to currently be behind the end of the spot length, so we don't have
        // to worry about where in the spot they need to line up.
        let idx = l.occupants.iter().enumerate().position(|(idx, x)| {
            x.is_none() && l.spots[idx].dist_along + map_model::PARKING_SPOT_LENGTH >= dist_along
        })?;
        Some(l.spots[idx].clone())
    }
}

#[derive(Serialize, Deserialize)]
struct ParkingLane {
    id: LaneID,
    spots: Vec<ParkingSpot>,
    occupants: Vec<Option<CarID>>,
}

// TODO the f64's prevent derivation
impl PartialEq for ParkingLane {
    fn eq(&self, other: &ParkingLane) -> bool {
        self.id == other.id && self.occupants == other.occupants
    }
}

impl Eq for ParkingLane {}

impl ParkingLane {
    fn new(l: &Lane) -> ParkingLane {
        if l.lane_type != LaneType::Parking {
            return ParkingLane {
                id: l.id,
                spots: Vec::new(),
                occupants: Vec::new(),
            };
        }

        ParkingLane {
            id: l.id,
            occupants: iter::repeat(None).take(l.number_parking_spots()).collect(),
            spots: (0..l.number_parking_spots())
                .map(|idx| {
                    let spot_start = map_model::PARKING_SPOT_LENGTH * (2.0 + idx as f64);
                    let (pos, angle) = l.dist_along(spot_start);
                    ParkingSpot {
                        parking_lane: l.id,
                        spot_idx: idx,
                        dist_along: spot_start,
                        pos,
                        angle,
                    }
                })
                .collect(),
        }
    }

    fn remove_parked_car(&mut self, car: CarID) {
        let idx = self.occupants.iter().position(|x| *x == Some(car)).unwrap();
        self.occupants[idx] = None;
    }

    fn get_draw_cars(&self, map: &Map, properties: &BTreeMap<CarID, Vehicle>) -> Vec<DrawCar> {
        self.occupants
            .iter()
            .enumerate()
            .filter_map(|(idx, maybe_id)| {
                maybe_id.and_then(|id| {
                    let vehicle = &properties[&id];
                    let (front, angle) = self.spots[idx].front_of_car(vehicle);
                    Some(DrawCar::new(
                        id,
                        vehicle,
                        None,
                        map,
                        front,
                        angle,
                        0.0 * si::M,
                    ))
                })
            })
            .collect()
    }

    fn is_empty(&self) -> bool {
        !self.occupants.iter().find(|&&x| x.is_some()).is_some()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Derivative)]
#[derivative(PartialEq, Eq)]
pub struct ParkingSpot {
    pub parking_lane: LaneID,
    pub spot_idx: usize,
    // These 3 are of the front of the parking spot
    #[derivative(PartialEq = "ignore")]
    dist_along: Distance,
    pos: Pt2D,
    #[derivative(PartialEq = "ignore")]
    angle: Angle,
}

impl ParkingSpot {
    pub fn dist_along_for_ped(&self) -> Distance {
        // Always centered in the entire parking spot
        self.dist_along - (map_model::PARKING_SPOT_LENGTH / 2.0)
    }

    pub fn dist_along_for_car(&self, vehicle: &Vehicle) -> Distance {
        // Find the offset to center this particular car in the parking spot
        let offset = (map_model::PARKING_SPOT_LENGTH - vehicle.length) / 2.0;
        self.dist_along - offset
    }

    fn front_of_car(&self, vehicle: &Vehicle) -> (Pt2D, Angle) {
        // Find the offset to center this particular car in the parking spot
        let offset = (map_model::PARKING_SPOT_LENGTH - vehicle.length) / 2.0;
        (
            self.pos
                .project_away(offset.value_unsafe, self.angle.opposite()),
            self.angle,
        )
    }
}
