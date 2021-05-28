use std::hash::{Hash, Hasher};
use std::cmp::Reverse;

use priority_queue::PriorityQueue;
use serde::{Deserialize, Serialize};

use abstutil::Counter;
use geom::{Duration, Histogram, Time};
use map_model::{BusRouteID, IntersectionID};

use crate::{
    pandemic, AgentID, CarID, CreateCar, CreatePedestrian, PedestrianID, StartTripArgs, TripID,
};

#[derive(Serialize, Deserialize, Clone)]
pub(crate) enum Command {
    /// If true, retry when there's no room to spawn somewhere
    SpawnCar(CreateCar, bool),
    SpawnPed(CreatePedestrian),
    StartTrip(TripID, StartTripArgs),
    UpdateCar(CarID),
    /// Distinguish this from UpdateCar to avoid confusing things
    UpdateLaggyHead(CarID),
    UpdatePed(PedestrianID),
    UpdateIntersection(IntersectionID),
    Callback(Duration),
    Pandemic(pandemic::Cmd),
    /// The Time is redundant, just used to dedupe commands
    StartBus(BusRouteID, Time),
}

impl Command {
    pub fn update_agent(id: AgentID) -> Command {
        match id {
            AgentID::Car(c) => Command::UpdateCar(c),
            AgentID::Pedestrian(p) => Command::UpdatePed(p),
            AgentID::BusPassenger(_, _) => unreachable!(),
        }
    }

    fn to_type(&self) -> CommandType {
        match self {
            Command::SpawnCar(ref create, _) => CommandType::Car(create.vehicle.id),
            Command::SpawnPed(ref create) => CommandType::Ped(create.id),
            Command::StartTrip(id, _) => CommandType::StartTrip(*id),
            Command::UpdateCar(id) => CommandType::Car(*id),
            Command::UpdateLaggyHead(id) => CommandType::CarLaggyHead(*id),
            Command::UpdatePed(id) => CommandType::Ped(*id),
            Command::UpdateIntersection(id) => CommandType::Intersection(*id),
            Command::Callback(_) => CommandType::Callback,
            Command::Pandemic(ref p) => CommandType::Pandemic(p.clone()),
            Command::StartBus(r, t) => CommandType::StartBus(*r, *t),
        }
    }

    fn to_simple_type(&self) -> SimpleCommandType {
        match self {
            Command::SpawnCar(_, _) => SimpleCommandType::Car,
            Command::SpawnPed(_) => SimpleCommandType::Ped,
            Command::StartTrip(_, _) => SimpleCommandType::StartTrip,
            Command::UpdateCar(_) => SimpleCommandType::Car,
            Command::UpdateLaggyHead(_) => SimpleCommandType::CarLaggyHead,
            Command::UpdatePed(_) => SimpleCommandType::Ped,
            Command::UpdateIntersection(_) => SimpleCommandType::Intersection,
            Command::Callback(_) => SimpleCommandType::Callback,
            Command::Pandemic(_) => SimpleCommandType::Pandemic,
            Command::StartBus(_, _) => SimpleCommandType::StartBus,
        }
    }
}

/// A smaller version of Command that satisfies many more properties. Only one Command per
/// CommandType may exist at a time.
#[derive(PartialEq, Eq, Hash, Debug)]
enum CommandType {
    StartTrip(TripID),
    Car(CarID),
    CarLaggyHead(CarID),
    Ped(PedestrianID),
    Intersection(IntersectionID),
    Callback,
    Pandemic(pandemic::Cmd),
    StartBus(BusRouteID, Time),
}

/// A more compressed form of CommandType, just used for keeping stats on event processing.
#[derive(PartialEq, Eq, Ord, PartialOrd, Clone, Debug)]
enum SimpleCommandType {
    StartTrip,
    Car,
    CarLaggyHead,
    Ped,
    Intersection,
    Callback,
    Pandemic,
    StartBus,
}

impl PartialEq for Command {
    fn eq(&self, other: &Command) -> bool {
        self.to_type() == other.to_type()
    }
}

impl Eq for Command {}

impl Hash for Command {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.to_type().hash(state);
    }
}

/// The priority queue driving the discrete event simulation. Different pieces of the simulation
/// schedule Commands to happen at a specific time, and the Scheduler hands out the commands in
/// order.
#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct Scheduler {
    // PriorityQueue returns the highest priorities first, so reverse the ordering on Time to get
    // earliest times.
    queue: PriorityQueue<Command, Reverse<Time>>,

    latest_time: Time,
    last_time: Time,
    #[serde(skip_serializing, skip_deserializing)]
    delta_times: Histogram<Duration>,
    #[serde(skip_serializing, skip_deserializing)]
    cmd_type_counts: Counter<SimpleCommandType>,
}

impl Scheduler {
    pub fn new() -> Scheduler {
        Scheduler {
            queue: PriorityQueue::new(),
            latest_time: Time::START_OF_DAY,
            last_time: Time::START_OF_DAY,
            delta_times: Histogram::new(),
            cmd_type_counts: Counter::new(),
        }
    }

    pub fn push(&mut self, time: Time, cmd: Command) {
        if time < self.latest_time {
            panic!(
                "It's at least {}, so can't schedule a command for {}",
                self.latest_time, time
            );
        }
        self.last_time = self.last_time.max(time);
        self.delta_times.add(time - self.latest_time);
        self.cmd_type_counts.inc(cmd.to_simple_type());

        // We're assuming the caller isn't double-scheduling the same command.
        self.queue.push(cmd, Reverse(time));
    }

    pub fn update(&mut self, new_time: Time, cmd: Command) {
        if new_time < self.latest_time {
            panic!(
                "It's at least {}, so can't schedule a command for {}",
                self.latest_time, new_time
            );
        }
        self.last_time = self.last_time.max(new_time);

        // There are a few possible implementations here; I haven't checked correctness carefully,
        // but the total performance of the alternatives seems roughly equivalent.

        // V1: Just push, priority_queue overwrites existing items seemingly
        self.push(new_time, cmd);

        // V2:
        // Note that change_priority doesn't insert the command if it's not already there. That's
        // why we use push_decrease.
        /*if self.queue.change_priority(&cmd, Reverse(new_time)).is_none() {
            // If the command wasn't even scheduled yet, go do that
            self.push(new_time, cmd);
        }*/


        // V3:
        // TODO Ahhhh the order matters. be very careful here.
        //self.queue.push_decrease(cmd, Reverse(new_time));
    }

    pub fn cancel(&mut self, cmd: Command) {
        // It's fine if a previous command hasn't actually been scheduled.
        self.queue.remove(&cmd);
    }

    /// This next command might've actually been rescheduled to a later time; the caller won't know
    /// that here.
    pub fn peek_next_time(&self) -> Option<Time> {
        self.queue.peek().map(|(_, t)| t.0)
    }

    pub fn get_last_time(&self) -> Time {
        self.last_time
    }

    pub fn get_next(&mut self) -> Command {
        let (cmd, time) = self.queue.pop().unwrap();
        self.latest_time = time.0;
        cmd
    }

    pub fn describe_stats(&self) -> Vec<String> {
        let mut stats = vec![
            format!("delta times for events: {}", self.delta_times.describe()),
            "count for each command type:".to_string(),
        ];
        for (cmd, cnt) in self.cmd_type_counts.borrow() {
            stats.push(format!("{:?}: {}", cmd, abstutil::prettyprint_usize(*cnt)));
        }
        stats
    }
}
