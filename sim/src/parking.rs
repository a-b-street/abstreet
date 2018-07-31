use draw_car;
use draw_car::DrawCar;
use map_model;
use map_model::{Lane, LaneID, LaneType, Map};
use rand::Rng;
use std::iter;
use CarID;

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
            l: id,
            spots: Vec::new(),
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
                if !spot.is_some() && rng.gen_bool(percent_capacity_to_fill) {
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

    pub fn get_last_parked_car(&self, id: LaneID) -> Option<CarID> {
        self.lanes[id.0].get_last_parked_car()
    }

    pub fn remove_last_parked_car(&mut self, id: LaneID, car: CarID) {
        self.lanes[id.0].remove_last_parked_car(car);
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
}

#[derive(Serialize, Deserialize, PartialEq, Eq)]
struct ParkingLane {
    l: LaneID,
    spots: Vec<Option<CarID>>,
}

impl ParkingLane {
    fn new(l: &Lane) -> ParkingLane {
        if l.lane_type != LaneType::Parking {
            return ParkingLane {
                l: l.id,
                spots: Vec::new(),
            };
        }

        ParkingLane {
            l: l.id,
            spots: iter::repeat(None).take(l.number_parking_spots()).collect(),
        }
    }

    fn get_last_parked_car(&self) -> Option<CarID> {
        self.spots
            .iter()
            .rfind(|&&x| x.is_some())
            .map(|l| l.unwrap())
    }

    fn remove_last_parked_car(&mut self, car: CarID) {
        let idx = self.spots
            .iter()
            .rposition(|&x| x.is_some())
            .expect("No parked cars at all now");
        assert_eq!(self.spots[idx], Some(car));
        self.spots[idx] = None;
    }

    fn get_draw_cars(&self, map: &Map) -> Vec<DrawCar> {
        let l = map.get_l(self.l);
        // TODO this is slow to do constantly! can we precompute for each spot or something like
        // that?
        self.spots
            .iter()
            .enumerate()
            .filter_map(|(idx, maybe_id)| {
                maybe_id.and_then(|id| {
                    let spot_start = map_model::PARKING_SPOT_LENGTH * (1.0 + idx as f64);
                    let (front, angle) = l.dist_along(
                        spot_start - (map_model::PARKING_SPOT_LENGTH - draw_car::CAR_LENGTH) / 2.0,
                    );
                    Some(DrawCar::new(id, None, map, front, angle))
                })
            })
            .collect()
    }

    pub fn is_empty(&self) -> bool {
        !self.get_last_parked_car().is_some()
    }
}
