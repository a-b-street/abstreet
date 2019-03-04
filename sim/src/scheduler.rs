use crate::{
    AgentID, CreateCar, CreatePedestrian, DrivingSimState, IntersectionSimState, ParkingSimState,
    TripManager, WalkingSimState,
};
use geom::Duration;
use map_model::Map;
use serde_derive::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub enum Command {
    SpawnCar(Duration, CreateCar),
    SpawnPed(Duration, CreatePedestrian),
}

impl Command {
    fn at(&self) -> Duration {
        match self {
            Command::SpawnCar(at, _) => *at,
            Command::SpawnPed(at, _) => *at,
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq)]
pub struct Scheduler {
    commands: PriorityQueue<Command>,
}

impl Scheduler {
    pub fn new() -> Scheduler {
        Scheduler {
            commands: PriorityQueue::new(),
        }
    }

    pub fn step_if_needed(
        &mut self,
        now: Duration,
        map: &Map,
        parking: &mut ParkingSimState,
        walking: &mut WalkingSimState,
        driving: &mut DrivingSimState,
        intersections: &IntersectionSimState,
        trips: &mut TripManager,
    ) {
        while let Some(cmd) = self.commands.get_next(now) {
            match cmd {
                Command::SpawnCar(_, create_car) => {
                    if driving.start_car_on_lane(now, create_car.clone(), map, intersections) {
                        trips.agent_starting_trip_leg(
                            AgentID::Car(create_car.vehicle.id),
                            create_car.trip,
                        );
                        if let Some(parked_car) = create_car.maybe_parked_car {
                            parking.remove_parked_car(parked_car);
                        }
                    } else {
                        self.enqueue_command(Command::SpawnCar(
                            now + Duration::EPSILON,
                            create_car,
                        ));
                    }
                }
                Command::SpawnPed(_, create_ped) => {
                    // Do the order a bit backwards so we don't have to clone the CreatePedestrian.
                    // spawn_ped can't fail.
                    trips.agent_starting_trip_leg(
                        AgentID::Pedestrian(create_ped.id),
                        create_ped.trip,
                    );
                    walking.spawn_ped(now, create_ped, map);
                }
            };
        }
    }

    pub fn enqueue_command(&mut self, cmd: Command) {
        self.commands.push(cmd.at(), cmd);
    }
}

#[derive(Serialize, Deserialize, PartialEq)]
pub struct PriorityQueue<I> {
    // TODO Implement more efficiently. Last element has earliest time.
    items: Vec<(Duration, I)>,
}

impl<I> PriorityQueue<I> {
    pub fn new() -> PriorityQueue<I> {
        PriorityQueue { items: Vec::new() }
    }

    pub fn push(&mut self, time: Duration, item: I) {
        // TODO Implement more efficiently
        self.items.push((time, item));
        self.items.sort_by_key(|(t, _)| *t);
        self.items.reverse();
    }

    // This API is safer than handing out a batch of items at a time, because while processing one
    // item, we might change the priority of other items or add new items. Don't make the caller
    // reconcile those changes -- just keep pulling items from here, one at a time.
    pub fn get_next(&mut self, now: Duration) -> Option<I> {
        let next_time = self.items.last().as_ref()?.0;
        // TODO Enable this validation after we're properly event-based. Right now, there are spawn
        // times between 0s and 0.1s, and stepping by 0.1s is too clunky.
        /*if next_time < now {
            panic!(
                "It's {}, but there's a command scheduled for {}",
                now, next_time
            );
        }*/
        if next_time > now {
            return None;
        }
        Some(self.items.pop().unwrap().1)
    }
}
