use std::cmp::Ordering;
use std::collections::hash_map::Entry;
use std::collections::{BinaryHeap, HashMap};

use serde::{Deserialize, Serialize};

use abstutil::Counter;
use geom::{Duration, Histogram, Time};
use map_model::{BusRouteID, IntersectionID};

use crate::{
    pandemic, AgentID, CarID, CreateCar, CreatePedestrian, PedestrianID, TripID, TripSpec,
};

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub(crate) enum Command {
    /// If true, retry when there's no room to spawn somewhere
    SpawnCar(CreateCar, bool),
    SpawnPed(CreatePedestrian),
    StartTrip(TripID, TripSpec),
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
#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord, Clone, Debug)]
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

#[derive(Serialize, Deserialize, PartialEq, Eq, Clone)]
struct Item {
    time: Time,
    cmd_type: CommandType,
}

impl PartialOrd for Item {
    fn partial_cmp(&self, other: &Item) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Item {
    fn cmp(&self, other: &Item) -> Ordering {
        // BinaryHeap is a max-heap, so reverse the comparison to get smallest times first.
        let ord = other.time.cmp(&self.time);
        if ord != Ordering::Equal {
            return ord;
        }
        // This is important! The tie-breaker if time is the same is ARBITRARY!
        self.cmd_type.cmp(&other.cmd_type)
    }
}

/// The priority queue driving the discrete event simulation. Different pieces of the simulation
/// schedule Commands to happen at a specific time, and the Scheduler hands out the commands in
/// order.
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Scheduler {
    items: BinaryHeap<Item>,
    queued_commands: HashMap<CommandType, (Command, Time)>,

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
            items: BinaryHeap::new(),
            queued_commands: HashMap::new(),
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

        let cmd_type = cmd.to_type();

        match self.queued_commands.entry(cmd_type.clone()) {
            Entry::Vacant(vacant) => {
                vacant.insert((cmd, time));
                self.items.push(Item { time, cmd_type });
            }
            Entry::Occupied(occupied) => {
                let (existing_cmd, existing_time) = occupied.get();
                panic!(
                    "Can't push({}, {:?}) because ({}, {:?}) already queued",
                    time, cmd, existing_time, existing_cmd
                );
            }
        }
    }

    pub fn update(&mut self, new_time: Time, cmd: Command) {
        if new_time < self.latest_time {
            panic!(
                "It's at least {}, so can't schedule a command for {}",
                self.latest_time, new_time
            );
        }
        self.last_time = self.last_time.max(new_time);

        let cmd_type = cmd.to_type();

        // It's fine if a previous command hasn't actually been scheduled.
        if let Some((existing_cmd, _)) = self.queued_commands.get(&cmd_type) {
            assert_eq!(cmd, *existing_cmd);
        }
        self.queued_commands
            .insert(cmd_type.clone(), (cmd, new_time));
        self.items.push(Item {
            time: new_time,
            cmd_type,
        });
    }

    pub fn cancel(&mut self, cmd: Command) {
        // It's fine if a previous command hasn't actually been scheduled.
        self.queued_commands.remove(&cmd.to_type());
    }

    /// This next command might've actually been rescheduled to a later time; the caller won't know
    /// that here.
    pub fn peek_next_time(&self) -> Option<Time> {
        self.items.peek().as_ref().map(|cmd| cmd.time)
    }

    pub fn get_last_time(&self) -> Time {
        self.last_time
    }

    /// This API is safer than handing out a batch of items at a time, because while processing one
    /// item, we might change the priority of other items or add new items. Don't make the caller
    /// reconcile those changes -- just keep pulling items from here, one at a time.
    //
    // TODO Above description is a little vague. This should be used with peek_next_time in a
    // particular way...
    pub fn get_next(&mut self) -> Option<Command> {
        let item = self.items.pop().unwrap();
        self.latest_time = item.time;
        match self.queued_commands.entry(item.cmd_type) {
            Entry::Vacant(_) => {
                // Command was cancelled
                return None;
            }
            Entry::Occupied(occupied) => {
                // Command was re-scheduled for later.
                if occupied.get().1 > item.time {
                    return None;
                }
                Some(occupied.remove().0)
            }
        }
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
