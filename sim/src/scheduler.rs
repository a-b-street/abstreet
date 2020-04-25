use crate::{
    pandemic, AgentID, CarID, CreateCar, CreatePedestrian, PedestrianID, TripID, TripSpec,
};
use derivative::Derivative;
use geom::{Duration, Histogram, Time};
use map_model::{IntersectionID, Path, PathRequest};
use serde_derive::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BinaryHeap};

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub enum Command {
    // If true, retry when there's no room to spawn somewhere
    SpawnCar(CreateCar, bool),
    SpawnPed(CreatePedestrian),
    StartTrip(TripID, TripSpec, Option<PathRequest>, Option<Path>),
    UpdateCar(CarID),
    // Distinguish this from UpdateCar to avoid confusing things
    UpdateLaggyHead(CarID),
    UpdatePed(PedestrianID),
    UpdateIntersection(IntersectionID),
    Savestate(Duration),
    Pandemic(pandemic::Cmd),
    FinishRemoteTrip(TripID),
}

impl Command {
    pub fn update_agent(id: AgentID) -> Command {
        match id {
            AgentID::Car(c) => Command::UpdateCar(c),
            AgentID::Pedestrian(p) => Command::UpdatePed(p),
        }
    }

    pub fn to_type(&self) -> CommandType {
        match self {
            Command::SpawnCar(ref create, _) => CommandType::Car(create.vehicle.id),
            Command::SpawnPed(ref create) => CommandType::Ped(create.id),
            Command::StartTrip(id, _, _, _) => CommandType::StartTrip(*id),
            Command::UpdateCar(id) => CommandType::Car(*id),
            Command::UpdateLaggyHead(id) => CommandType::CarLaggyHead(*id),
            Command::UpdatePed(id) => CommandType::Ped(*id),
            Command::UpdateIntersection(id) => CommandType::Intersection(*id),
            Command::Savestate(_) => CommandType::Savestate,
            Command::Pandemic(ref p) => CommandType::Pandemic(p.clone()),
            Command::FinishRemoteTrip(t) => CommandType::FinishRemoteTrip(*t),
        }
    }
}

// A smaller version of Command that satisfies many more properties. Only one Command per
// CommandType may exist at a time.
#[derive(Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Clone, Debug)]
pub enum CommandType {
    StartTrip(TripID),
    Car(CarID),
    CarLaggyHead(CarID),
    Ped(PedestrianID),
    Intersection(IntersectionID),
    Savestate,
    Pandemic(pandemic::Cmd),
    FinishRemoteTrip(TripID),
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

#[derive(Clone, Serialize, Deserialize, Derivative)]
#[derivative(PartialEq)]
pub struct Scheduler {
    // TODO Argh, really?!
    #[derivative(PartialEq = "ignore")]
    items: BinaryHeap<Item>,
    queued_commands: BTreeMap<CommandType, (Command, Time)>,

    latest_time: Time,
    last_time: Time,
    #[derivative(PartialEq = "ignore")]
    #[serde(skip_serializing, skip_deserializing)]
    delta_times: Histogram<Duration>,
}

impl Scheduler {
    pub fn new() -> Scheduler {
        Scheduler {
            items: BinaryHeap::new(),
            queued_commands: BTreeMap::new(),
            latest_time: Time::START_OF_DAY,
            last_time: Time::START_OF_DAY,
            delta_times: Histogram::new(),
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

        let cmd_type = cmd.to_type();

        // TODO Combo with entry API
        if let Some((existing_cmd, existing_time)) = self.queued_commands.get(&cmd_type) {
            panic!(
                "Can't push({}, {:?}) because ({}, {:?}) already queued",
                time, cmd, existing_time, existing_cmd
            );
        }
        self.queued_commands.insert(cmd_type.clone(), (cmd, time));
        self.items.push(Item { time, cmd_type });
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

    // TODO Should panic if a command of this type isn't scheduled. But currently failing
    // unexpectedly.
    pub fn must_cancel_by_type(&mut self, cmd: CommandType) {
        if self.queued_commands.remove(&cmd).is_none() {
            println!(
                "must_cancel_by_type({:?}) didn't find a matching command",
                cmd
            );
        }
    }

    // This next command might've actually been rescheduled to a later time; the caller won't know
    // that here.
    pub fn peek_next_time(&self) -> Option<Time> {
        self.items.peek().as_ref().map(|cmd| cmd.time)
    }

    pub fn get_last_time(&self) -> Time {
        self.last_time
    }

    // This API is safer than handing out a batch of items at a time, because while processing one
    // item, we might change the priority of other items or add new items. Don't make the caller
    // reconcile those changes -- just keep pulling items from here, one at a time.
    //
    // TODO Above description is a little vague. This should be used with peek_next_time in a
    // particular way...
    pub fn get_next(&mut self) -> Option<Command> {
        let item = self.items.pop().unwrap();
        self.latest_time = item.time;
        let (_, cmd_time) = self.queued_commands.get(&item.cmd_type)?;
        // Command was re-scheduled for later.
        if *cmd_time > item.time {
            return None;
        }
        let (cmd, _) = self.queued_commands.remove(&item.cmd_type)?;
        Some(cmd)
    }

    pub fn describe_stats(&self) -> String {
        format!("delta times for events: {}", self.delta_times.describe())
    }

    // It's much more efficient to save without the paths, and to recalculate them when loading
    // later.
    // TODO Why not just implement Default on Path and use skip_serializing? Because we want to
    // serialize paths inside Router for live agents. We need to defer calling make_router and just
    // store the input in CreateCar.
    // TODO Rethink all of this; probably broken by StartTrip.
    pub fn get_requests_for_savestate(&self) -> Vec<PathRequest> {
        let mut reqs = Vec::new();
        for (cmd, _) in self.queued_commands.values() {
            match cmd {
                Command::SpawnCar(ref create_car, _) => {
                    reqs.push(create_car.req.clone());
                }
                Command::SpawnPed(ref create_ped) => {
                    reqs.push(create_ped.req.clone());
                }
                _ => {}
            }
        }
        reqs
    }

    pub fn before_savestate(&mut self) -> Vec<Path> {
        let mut restore = Vec::new();
        for (cmd, _) in self.queued_commands.values_mut() {
            match cmd {
                Command::SpawnCar(ref mut create_car, _) => {
                    restore.push(
                        create_car
                            .router
                            .replace_path_for_serialization(Path::dummy()),
                    );
                }
                Command::SpawnPed(ref mut create_ped) => {
                    restore.push(std::mem::replace(&mut create_ped.path, Path::dummy()));
                }
                _ => {}
            }
        }
        restore
    }

    pub fn after_savestate(&mut self, mut restore: Vec<Path>) {
        restore.reverse();
        for (cmd, _) in self.queued_commands.values_mut() {
            match cmd {
                Command::SpawnCar(ref mut create_car, _) => {
                    create_car
                        .router
                        .replace_path_for_serialization(restore.pop().unwrap());
                }
                Command::SpawnPed(ref mut create_ped) => {
                    std::mem::replace(&mut create_ped.path, restore.pop().unwrap());
                }
                _ => {}
            }
        }
        assert!(restore.is_empty());
    }
}
