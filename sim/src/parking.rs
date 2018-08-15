use dimensioned::si;
use draw_car;
use draw_car::DrawCar;
use geom::{Angle, Pt2D};
use map_model;
use map_model::{Lane, LaneID, LaneType, Map};
use rand::Rng;
use sim::{CarStateTransitions, ParkingSpot};
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
            spot_fronts: Vec::new(),
        };
    }

    pub fn edit_add_lane(&mut self, l: &Lane) {
        self.lanes[l.id.0] = ParkingLane::new(l);
    }

    pub fn total_count(&self) -> usize {
        self.total_count
    }

    // Kind of vague whether this should handle existing spots or not
    pub fn seed_random_cars<R: Rng + ?Sized>(
        &mut self,
        rng: &mut R,
        percent_capacity_to_fill: f64,
        id_counter: &mut usize,
    ) {
        assert!(percent_capacity_to_fill >= 0.0 && percent_capacity_to_fill <= 1.0);

        let mut total_capacity = 0;
        for l in &self.lanes {
            total_capacity += l.spots.len();
        }

        let mut new_cars = 0;
        for l in &mut self.lanes {
            for spot in &mut l.spots {
                if spot.is_none() && rng.gen_bool(percent_capacity_to_fill) {
                    new_cars += 1;
                    *spot = Some(CarID(*id_counter));
                    *id_counter += 1;
                }
            }
        }
        self.total_count += new_cars;
        println!(
            "Seeded {} of {} parking spots with cars",
            new_cars, total_capacity
        );
    }

    pub fn remove_parked_car(&mut self, id: LaneID, car: CarID) {
        self.lanes[id.0].remove_parked_car(car);
        self.total_count -= 1;
    }

    pub fn get_draw_cars(&self, id: LaneID, map: &Map) -> Vec<DrawCar> {
        self.lanes[id.0].get_draw_cars(map)
    }

    pub fn get_draw_car(&self, id: CarID, map: &Map) -> Option<DrawCar> {
        // TODO this is so horrendously slow :D
        for l in &self.lanes {
            if l.spots.contains(&Some(id)) {
                return l.get_draw_cars(map).into_iter().find(|c| c.id == id);
            }
        }
        None
    }

    pub fn lane_of_car(&self, id: CarID) -> Option<LaneID> {
        // TODO this is so horrendously slow :D
        for l in &self.lanes {
            if l.spots.contains(&Some(id)) {
                return Some(l.id);
            }
        }
        None
    }

    // Of the front of the car
    pub fn get_spot_of_car(&self, c: CarID, l: LaneID) -> ParkingSpot {
        let idx = self.lanes[l.0]
            .spots
            .iter()
            .position(|x| *x == Some(c))
            .unwrap();
        ParkingSpot {
            parking_lane: l,
            spot_idx: idx,
            dist_along: self.lanes[l.0].spot_fronts[idx].0,
        }
    }

    pub fn get_all_cars(&self) -> Vec<(CarID, LaneID)> {
        let mut result = Vec::new();
        for l in &self.lanes {
            for maybe_car in &l.spots {
                if let Some(car) = maybe_car {
                    result.push((*car, l.id));
                }
            }
        }
        result
    }

    pub fn handle_transitions(&mut self, transitions: CarStateTransitions) {
        for p in transitions.finished_parking {
            assert_eq!(
                self.lanes[p.spot.parking_lane.0].spots[p.spot.spot_idx],
                None
            );
            self.lanes[p.spot.parking_lane.0].spots[p.spot.spot_idx] = Some(p.car);
        }
    }

    pub fn get_first_free_spot(&self, lane: LaneID, dist_along: Distance) -> Option<ParkingSpot> {
        let l = &self.lanes[lane.0];
        let idx = l.spots
            .iter()
            .enumerate()
            .position(|(idx, x)| x.is_none() && l.spot_fronts[idx].0 >= dist_along)?;
        Some(ParkingSpot {
            parking_lane: lane,
            spot_idx: idx,
            dist_along: l.spot_fronts[idx].0,
        })
    }
}

#[derive(Serialize, Deserialize)]
struct ParkingLane {
    id: LaneID,
    spots: Vec<Option<CarID>>,
    spot_fronts: Vec<(Distance, Pt2D, Angle)>,
}

// TODO the f64's prevent derivation
impl PartialEq for ParkingLane {
    fn eq(&self, other: &ParkingLane) -> bool {
        self.id == other.id && self.spots == other.spots
    }
}

impl Eq for ParkingLane {}

impl ParkingLane {
    fn new(l: &Lane) -> ParkingLane {
        if l.lane_type != LaneType::Parking {
            return ParkingLane {
                id: l.id,
                spots: Vec::new(),
                spot_fronts: Vec::new(),
            };
        }

        ParkingLane {
            id: l.id,
            spots: iter::repeat(None).take(l.number_parking_spots()).collect(),
            spot_fronts: (0..l.number_parking_spots())
                .map(|idx| {
                    let spot_start = map_model::PARKING_SPOT_LENGTH * (1.0 + idx as f64);
                    let dist_along =
                        spot_start - (map_model::PARKING_SPOT_LENGTH - draw_car::CAR_LENGTH) / 2.0;
                    let (pos, angle) = l.dist_along(dist_along);
                    (dist_along, pos, angle)
                })
                .collect(),
        }
    }

    fn remove_parked_car(&mut self, car: CarID) {
        let idx = self.spots.iter().position(|x| *x == Some(car)).unwrap();
        self.spots[idx] = None;
    }

    fn get_draw_cars(&self, map: &Map) -> Vec<DrawCar> {
        self.spots
            .iter()
            .enumerate()
            .filter_map(|(idx, maybe_id)| {
                maybe_id.and_then(|id| {
                    let (_, front, angle) = self.spot_fronts[idx];
                    Some(DrawCar::new(id, None, map, front, angle, 0.0 * si::M))
                })
            })
            .collect()
    }

    fn is_empty(&self) -> bool {
        !self.spots.iter().find(|&&x| x.is_some()).is_some()
    }
}
