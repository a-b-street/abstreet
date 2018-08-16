use map_model::LaneID;
use parking::ParkingSimState;
use rand::Rng;
use sim::CarParking;
use {CarID, PedestrianID, Tick};

// TODO move the stuff in sim that does RNG stuff, picks goals, etc to here. make the UI commands
// funnel into here and do stuff on the next tick.

#[derive(Serialize, Deserialize, PartialEq, Eq)]
enum Command {
    // goal lane
    StartParkedCar(Tick, CarID, LaneID),
    // start, goal lanes
    SpawnPedestrian(Tick, PedestrianID, LaneID, LaneID),
}

// This must get the car/ped IDs correct.
#[derive(Serialize, Deserialize, PartialEq, Eq)]
pub struct Spawner {
    // This happens immediately (at the beginning of the simulation in most cases, except for
    // interactive UI stuff)
    spawn_parked_cars: Vec<CarParking>,

    // Ordered by time
    commands: Vec<Command>,

    car_id_counter: usize,
}

impl Spawner {
    pub fn empty() -> Spawner {
        Spawner {
            spawn_parked_cars: Vec::new(),
            commands: Vec::new(),
            car_id_counter: 0,
        }
    }

    pub fn step(&mut self, _time: Tick, parking_sim: &mut ParkingSimState) {
        for p in self.spawn_parked_cars.drain(0..) {
            parking_sim.add_parked_car(p);
        }
    }

    // TODO the mut is temporary
    pub fn seed_parked_cars<R: Rng + ?Sized>(
        &mut self,
        percent_capacity_to_fill: f64,
        parking_sim: &mut ParkingSimState,
        rng: &mut R,
    ) {
        assert!(percent_capacity_to_fill >= 0.0 && percent_capacity_to_fill <= 1.0);
        assert!(self.spawn_parked_cars.is_empty());

        let mut total_capacity = 0;
        let mut new_cars = 0;
        for spot in parking_sim.get_all_free_spots() {
            total_capacity += 1;
            if rng.gen_bool(percent_capacity_to_fill) {
                new_cars += 1;
                // TODO since spawning applies during the next step, lots of stuff breaks without
                // this :(
                parking_sim.add_parked_car(CarParking::new(CarID(self.car_id_counter), spot));
                //self.spawn_parked_cars.push(CarParking::new(CarID(self.car_id_counter), spot));
                self.car_id_counter += 1;
            }
        }

        println!(
            "Seeded {} of {} parking spots with cars",
            new_cars, total_capacity
        );
    }
}
