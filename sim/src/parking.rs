use map_model::{LaneType, Map, Road, RoadID};
use rand::Rng;
use std::iter;
use CarID;

#[derive(Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ParkingSimState {
    // TODO hacky, but other types of lanes just mark 0 spots. :\
    roads: Vec<ParkingLane>,
}

impl ParkingSimState {
    pub(crate) fn new(map: &Map) -> ParkingSimState {
        ParkingSimState {
            roads: map.all_roads()
                .iter()
                .map(|r| ParkingLane::new(r))
                .collect(),
        }
    }

    // Kind of vague whether this should handle existing spots or not
    pub(crate) fn seed_random_cars<R: Rng + ?Sized>(
        &mut self,
        rng: &mut R,
        percent_capacity_to_fill: f64,
        id_counter: &mut usize,
    ) {
        assert!(percent_capacity_to_fill >= 0.0 && percent_capacity_to_fill <= 1.0);

        let mut total_capacity = 0;
        for r in &self.roads {
            total_capacity += r.spots.len();
        }

        let mut new_cars = 0;
        for r in &mut self.roads {
            for spot in &mut r.spots {
                if !spot.is_some() && rng.gen_bool(percent_capacity_to_fill) {
                    new_cars += 1;
                    *spot = Some(CarID(*id_counter));
                    *id_counter += 1;
                }
            }
        }
        println!(
            "Seeded {} of {} parking spots with cars",
            new_cars, total_capacity
        );
    }

    pub(crate) fn get_last_parked_car(&self, id: RoadID) -> Option<CarID> {
        self.roads[id.0].get_last_parked_car()
    }

    pub(crate) fn remove_last_parked_car(&mut self, id: RoadID, car: CarID) {
        self.roads[id.0].remove_last_parked_car(car)
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq)]
struct ParkingLane {
    r: RoadID,
    spots: Vec<Option<CarID>>,
}

impl ParkingLane {
    fn new(r: &Road) -> ParkingLane {
        if r.lane_type != LaneType::Parking {
            return ParkingLane {
                r: r.id,
                spots: Vec::new(),
            };
        }

        ParkingLane {
            r: r.id,
            spots: iter::repeat(None).take(r.number_parking_spots()).collect(),
        }
    }

    fn get_last_parked_car(&self) -> Option<CarID> {
        self.spots
            .iter()
            .rfind(|&&x| x.is_some())
            .map(|r| r.unwrap())
    }

    fn remove_last_parked_car(&mut self, car: CarID) {
        let idx = self.spots
            .iter()
            .rposition(|&x| x.is_some())
            .expect("No parked cars at all now");
        assert_eq!(self.spots[idx], Some(car));
        self.spots[idx] = None;
    }
}
