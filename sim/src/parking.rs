use geom::{Angle, Pt2D};
use kinematics::Vehicle;
use map_model;
use map_model::{BuildingID, Lane, LaneID, LaneType, Map, Position, Traversable};
use std::collections::HashSet;
use std::iter;
use {CarID, CarState, Distance, DrawCarInput, ParkedCar, ParkingSpot, VehicleType};

#[derive(Serialize, Deserialize, PartialEq)]
pub struct ParkingSimState {
    // TODO hacky, but other types of lanes just mark 0 spots. :\
    lanes: Vec<ParkingLane>,
    total_count: usize,
}

impl ParkingSimState {
    pub fn new(map: &Map) -> ParkingSimState {
        ParkingSimState {
            lanes: map
                .all_lanes()
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

    pub fn get_free_spots(&self, lane: LaneID) -> Vec<ParkingSpot> {
        let l = &self.lanes[lane.0];
        let mut spots: Vec<ParkingSpot> = Vec::new();
        for (idx, maybe_occupant) in l.occupants.iter().enumerate() {
            if maybe_occupant.is_none() {
                spots.push(ParkingSpot::new(l.id, idx));
            }
        }
        spots
    }

    pub fn remove_parked_car(&mut self, p: ParkedCar) {
        self.lanes[p.spot.lane.0].remove_parked_car(p);
        self.total_count -= 1;
    }

    pub fn add_parked_car(&mut self, p: ParkedCar) {
        let spot = p.spot;
        assert_eq!(self.lanes[spot.lane.0].occupants[spot.idx], None);
        self.lanes[spot.lane.0].occupants[spot.idx] = Some(p);
        self.total_count += 1;
    }

    pub fn get_draw_cars(&self, id: LaneID) -> Vec<DrawCarInput> {
        self.lanes[id.0].get_draw_cars()
    }

    pub fn get_draw_car(&self, id: CarID) -> Option<DrawCarInput> {
        // TODO this is so horrendously slow :D
        for l in &self.lanes {
            if l.occupants.iter().find(|x| is_car(x, id)).is_some() {
                return l.get_draw_cars().into_iter().find(|c| c.id == id);
            }
        }
        None
    }

    pub fn get_all_draw_cars(&self) -> Vec<DrawCarInput> {
        // TODO this is so horrendously slow :D
        let mut cars: Vec<DrawCarInput> = Vec::new();
        for l in &self.lanes {
            cars.extend(l.get_draw_cars());
        }
        cars
    }

    pub fn lookup_car(&self, id: CarID) -> Option<&ParkedCar> {
        // TODO this is so horrendously slow :D
        for l in &self.lanes {
            if let Some(p) = l.occupants.iter().find(|x| is_car(x, id)) {
                return p.as_ref();
            }
        }
        None
    }

    pub fn get_first_free_spot(&self, parking_pos: Position) -> Option<ParkingSpot> {
        let l = &self.lanes[parking_pos.lane().0];
        // Just require the car to currently be behind the end of the spot length, so we don't have
        // to worry about where in the spot they need to line up.
        let idx = l.occupants.iter().enumerate().position(|(idx, x)| {
            x.is_none()
                && l.spots[idx].dist_along + map_model::PARKING_SPOT_LENGTH
                    >= parking_pos.dist_along()
        })?;
        Some(ParkingSpot::new(parking_pos.lane(), idx))
    }

    pub fn get_car_at_spot(&self, spot: ParkingSpot) -> Option<ParkedCar> {
        let l = &self.lanes[spot.lane.0];
        l.occupants[spot.idx].clone()
    }

    pub fn spot_to_driving_pos(
        &self,
        spot: ParkingSpot,
        vehicle: &Vehicle,
        driving_lane: LaneID,
        map: &Map,
    ) -> Position {
        Position::new(spot.lane, self.get_spot(spot).dist_along_for_car(vehicle))
            .equiv_pos(driving_lane, map)
    }

    pub fn spot_to_sidewalk_pos(&self, spot: ParkingSpot, sidewalk: LaneID, map: &Map) -> Position {
        Position::new(spot.lane, self.get_spot(spot).dist_along_for_ped()).equiv_pos(sidewalk, map)
    }

    fn get_spot(&self, spot: ParkingSpot) -> &ParkingSpotGeometry {
        &self.lanes[spot.lane.0].spots[spot.idx]
    }

    pub fn tooltip_lines(&self, id: CarID) -> Vec<String> {
        let c = self.lookup_car(id).unwrap();
        vec![format!("{} is parked, owned by {:?}", c.car, c.owner)]
    }

    pub fn get_parked_cars_by_owner(&self, id: BuildingID) -> Vec<&ParkedCar> {
        let mut result: Vec<&ParkedCar> = Vec::new();
        for l in &self.lanes {
            for maybe_occupant in &l.occupants {
                if let Some(o) = maybe_occupant {
                    if o.owner == Some(id) {
                        result.push(maybe_occupant.as_ref().unwrap());
                    }
                }
            }
        }
        result
    }

    pub fn get_owner_of_car(&self, id: CarID) -> Option<BuildingID> {
        self.lookup_car(id).and_then(|p| p.owner)
    }

    pub fn count(&self, lanes: &HashSet<LaneID>) -> (usize, usize) {
        let mut cars_parked = 0;
        let mut open_parking_spots = 0;

        for id in lanes {
            for maybe_car in &self.lanes[id.0].occupants {
                if maybe_car.is_some() {
                    cars_parked += 1;
                } else {
                    open_parking_spots += 1;
                }
            }
        }

        (cars_parked, open_parking_spots)
    }
}

#[derive(Serialize, Deserialize, PartialEq)]
struct ParkingLane {
    id: LaneID,
    spots: Vec<ParkingSpotGeometry>,
    occupants: Vec<Option<ParkedCar>>,
}

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
                    ParkingSpotGeometry {
                        dist_along: spot_start,
                        pos,
                        angle,
                    }
                }).collect(),
        }
    }

    fn remove_parked_car(&mut self, p: ParkedCar) {
        let match_against = Some(p.clone());
        let idx = self
            .occupants
            .iter()
            .position(|x| *x == match_against)
            .unwrap();
        self.occupants[idx] = None;
    }

    fn get_draw_cars(&self) -> Vec<DrawCarInput> {
        self.occupants
            .iter()
            .filter_map(|maybe_occupant| {
                maybe_occupant.as_ref().and_then(|p| {
                    let (front, angle) = self.spots[p.spot.idx].front_of_car(&p.vehicle);
                    Some(DrawCarInput {
                        id: p.car,
                        vehicle_length: p.vehicle.length,
                        waiting_for_turn: None,
                        front: front,
                        angle: angle,
                        stopping_trace: None,
                        state: CarState::Parked,
                        vehicle_type: VehicleType::Car,
                        on: Traversable::Lane(self.id),
                    })
                })
            }).collect()
    }

    fn is_empty(&self) -> bool {
        !self.occupants.iter().find(|x| x.is_some()).is_some()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct ParkingSpotGeometry {
    // These 3 are of the front of the parking spot
    dist_along: Distance,
    pos: Pt2D,
    angle: Angle,
}

impl ParkingSpotGeometry {
    fn dist_along_for_ped(&self) -> Distance {
        // Always centered in the entire parking spot
        self.dist_along - (map_model::PARKING_SPOT_LENGTH / 2.0)
    }

    fn dist_along_for_car(&self, vehicle: &Vehicle) -> Distance {
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

fn is_car(maybe_occupant: &&Option<ParkedCar>, car: CarID) -> bool {
    match maybe_occupant {
        Some(p) => p.car == car,
        None => false,
    }
}
