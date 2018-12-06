use crate::driving::{CreateCar, DrivingSimState};
use crate::events::Event;
use crate::parking::ParkingSimState;
use crate::trips::TripManager;
use crate::walking::{CreatePedestrian, WalkingSimState};
use crate::{AgentID, Tick};
use map_model::Map;
use serde_derive::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, PartialEq)]
pub enum Command {
    SpawnCar(Tick, CreateCar),
    SpawnPed(Tick, CreatePedestrian),
}

impl Command {
    fn at(&self) -> Tick {
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

    pub fn step(
        &mut self,
        events: &mut Vec<Event>,
        now: Tick,
        map: &Map,
        parking_sim: &mut ParkingSimState,
        walking_sim: &mut WalkingSimState,
        driving_sim: &mut DrivingSimState,
        trips: &mut TripManager,
    ) {
        let mut this_tick_commands: Vec<Command> = Vec::new();
        loop {
            if self
                .commands
                .last()
                .and_then(|cmd| Some(now == cmd.at()))
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
                    if driving_sim.start_car_on_lane(events, now, map, create_car.clone()) {
                        trips
                            .agent_starting_trip_leg(AgentID::Car(create_car.car), create_car.trip);
                        if let Some(parked_car) = create_car.maybe_parked_car {
                            parking_sim.remove_parked_car(parked_car);
                        }
                    } else {
                        self.enqueue_command(Command::SpawnCar(now.next(), create_car));
                    }
                }
                Command::SpawnPed(_, create_ped) => {
                    // Do the order a bit backwards so we don't have to clone the CreatePedestrian.
                    // seed_pedestrian can't fail.
                    trips.agent_starting_trip_leg(
                        AgentID::Pedestrian(create_ped.id),
                        create_ped.trip,
                    );
                    walking_sim.seed_pedestrian(events, now, create_ped);
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
        // Note the reverse sorting
        self.commands.sort_by(|a, b| b.at().cmp(&a.at()));
    }
}
