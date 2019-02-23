use crate::plugins::sim::new_des_model::{
    CreateCar, CreatePedestrian, DrivingSimState, IntersectionSimState, ParkingSimState,
    TripManager, WalkingSimState,
};
use geom::Duration;
use map_model::Map;
use serde_derive::{Deserialize, Serialize};
use sim::AgentID;

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
    // Ordered descending by time
    commands: Vec<Command>,
}

impl Scheduler {
    pub fn new() -> Scheduler {
        Scheduler {
            commands: Vec::new(),
        }
    }

    pub fn step_if_needed(
        &mut self,
        now: Duration,
        map: &Map,
        parking_sim: &mut ParkingSimState,
        walking_sim: &mut WalkingSimState,
        driving_sim: &mut DrivingSimState,
        intersections: &IntersectionSimState,
        trips: &mut TripManager,
    ) {
        let mut this_tick_commands: Vec<Command> = Vec::new();
        loop {
            if self
                .commands
                .last()
                // TODO >= just to handle the fact that we dont step on 0
                .and_then(|cmd| Some(now >= cmd.at()))
                .unwrap_or(false)
            {
                this_tick_commands.push(self.commands.pop().unwrap());
            } else {
                break;
            }
        }
        if this_tick_commands.is_empty() {
            return;
        }

        for cmd in this_tick_commands.into_iter() {
            match cmd {
                Command::SpawnCar(_, create_car) => {
                    if driving_sim.start_car_on_lane(now, map, create_car.clone(), intersections) {
                        trips.agent_starting_trip_leg(
                            AgentID::Car(create_car.vehicle.id),
                            create_car.trip,
                        );
                        if let Some(parked_car) = create_car.maybe_parked_car {
                            parking_sim.remove_parked_car(parked_car);
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
                    walking_sim.spawn_ped(now, create_ped, map);
                }
            };
        }
    }

    pub fn is_done(&self) -> bool {
        self.commands.is_empty()
    }

    pub fn enqueue_command(&mut self, cmd: Command) {
        // TODO Use some kind of priority queue that's serializable
        self.commands.push(cmd);
        self.commands.sort_by_key(|cmd| cmd.at());
        self.commands.reverse();
    }
}
