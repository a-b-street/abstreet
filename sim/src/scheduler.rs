use std::collections::hash_map::Entry;
use std::collections::{BinaryHeap, HashMap};

use serde::{Deserialize, Serialize};

use abstutil::{Counter, PriorityQueueItem};
use geom::{Duration, Histogram, Time};
use map_model::{IntersectionID, TransitRouteID};

use crate::{
    pandemic, AgentID, CarID, CreateCar, CreatePedestrian, PedestrianID, StartTripArgs, TripID,
};

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
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
    StartBus(TransitRouteID, Time),
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
    StartBus(TransitRouteID, Time),
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

/// The priority queue driving the discrete event simulation. Different pieces of the simulation
/// schedule Commands to happen at a specific time, and the Scheduler hands out the commands in
/// order.
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Scheduler {
    items: BinaryHeap<PriorityQueueItem<Time, CommandType>>,
    queued_commands: HashMap<CommandType, (Command, Time)>,

    latest_time: Time,
    last_time: Time,
    #[serde(skip_serializing, skip_deserializing)]
    delta_times: Histogram<Duration>,
    #[serde(skip_serializing, skip_deserializing)]
    cmd_type_counts: Counter<SimpleCommandType>,
    
    // New fields for cleanup optimization
    #[serde(skip_serializing, skip_deserializing)]
    stale_entries: usize,
    #[serde(skip_serializing, skip_deserializing)]
    operations_since_cleanup: usize,
}

const CLEANUP_THRESHOLD: usize = 10000;  // Clean up after this many operations
const STALE_RATIO_THRESHOLD: f64 = 0.3;  // Clean up if more than 30% of entries are stale

impl Scheduler {
    pub fn new() -> Scheduler {
        Scheduler {
            items: BinaryHeap::new(),
            queued_commands: HashMap::new(),
            latest_time: Time::START_OF_DAY,
            last_time: Time::START_OF_DAY,
            delta_times: Histogram::new(),
            cmd_type_counts: Counter::new(),
            stale_entries: 0,
            operations_since_cleanup: 0,
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
                self.items.push(PriorityQueueItem {
                    cost: time,
                    value: cmd_type,
                });
            }
            Entry::Occupied(occupied) => {
                let (existing_cmd, existing_time) = occupied.get();
                panic!(
                    "Can't push({}, {:?}) because ({}, {:?}) already queued",
                    time, cmd, existing_time, existing_cmd
                );
            }
        }
        
        self.operations_since_cleanup += 1;
        self.maybe_cleanup();
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
            // If there was a previous command, it's now stale
            self.stale_entries += 1;
        }
        self.queued_commands
            .insert(cmd_type.clone(), (cmd, new_time));
        self.items.push(PriorityQueueItem {
            cost: new_time,
            value: cmd_type,
        });
        
        self.operations_since_cleanup += 1;
        self.maybe_cleanup();
    }

    pub fn cancel(&mut self, cmd: Command) {
        // It's fine if a previous command hasn't actually been scheduled.
        if self.queued_commands.remove(&cmd.to_type()).is_some() {
            self.stale_entries += 1;
        }
        
        self.operations_since_cleanup += 1;
        self.maybe_cleanup();
    }

    /// This next command might've actually been rescheduled to a later time; the caller won't know
    /// that here.
    pub fn peek_next_time(&self) -> Option<Time> {
        self.items.peek().as_ref().map(|cmd| cmd.cost)
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
        loop {
            let item = self.items.pop()?;
            match self.queued_commands.entry(item.value) {
                Entry::Vacant(_) => {
                    // Command was cancelled, this is a stale entry
                    self.stale_entries = self.stale_entries.saturating_sub(1);
                    continue;
                }
                Entry::Occupied(occupied) => {
                    // Command was re-scheduled for later.
                    if occupied.get().1 > item.cost {
                        self.stale_entries = self.stale_entries.saturating_sub(1);
                        continue;
                    }
                    // Only update latest_time for valid commands
                    self.latest_time = item.cost;
                    self.operations_since_cleanup += 1;
                    return Some(occupied.remove().0);
                }
            }
        }
    }

    pub fn describe_stats(&self) -> Vec<String> {
        let mut stats = vec![
            format!("delta times for events: {}", self.delta_times.describe()),
            format!("heap size: {}, active commands: {}, stale entries: {}", 
                    self.items.len(), self.queued_commands.len(), self.stale_entries),
            "count for each command type:".to_string(),
        ];
        for (cmd, cnt) in self.cmd_type_counts.borrow() {
            stats.push(format!("{:?}: {}", cmd, abstutil::prettyprint_usize(*cnt)));
        }
        stats
    }
    
    /// Check if we should clean up stale entries
    fn maybe_cleanup(&mut self) {
        let should_cleanup = self.operations_since_cleanup >= CLEANUP_THRESHOLD ||
            (self.stale_entries > 100 && 
             self.stale_entries as f64 / self.items.len().max(1) as f64 > STALE_RATIO_THRESHOLD);
        
        if should_cleanup {
            self.cleanup();
        }
    }
    
    /// Remove stale entries from the heap
    fn cleanup(&mut self) {
        let old_size = self.items.len();
        
        // Rebuild the heap with only valid entries
        let valid_items: Vec<_> = self.items
            .drain()
            .filter(|item| {
                self.queued_commands.get(&item.value)
                    .map(|(_, time)| *time == item.cost)
                    .unwrap_or(false)
            })
            .collect();
        
        self.items = BinaryHeap::from(valid_items);
        self.stale_entries = 0;
        self.operations_since_cleanup = 0;
        
        let removed = old_size - self.items.len();
        if removed > 0 {
            log::debug!("Scheduler cleanup: removed {} stale entries", removed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CarID, PedestrianID, VehicleType};
    use geom::{Duration, Time};
    use std::time::Instant;

    #[test]
    fn test_scheduler_performance() {
        println!("\nRunning scheduler performance test...");
        
        // Run with different sizes to see scaling
        let test_sizes = vec![10_000, 50_000, 100_000];
        
        for size in test_sizes {
            println!("\nTesting with {} operations:", size);
            let duration = benchmark_scheduler(size);
            println!("  Time: {:?}", duration);
        }
    }

    fn benchmark_scheduler(num_operations: usize) -> std::time::Duration {
        let start = Instant::now();
        
        let mut scheduler = Scheduler::new();
        
        // Push initial commands
        for i in 0..num_operations {
            let time = Time::START_OF_DAY + Duration::seconds(i as f64 / 100.0);
            
            let cmd = match i % 4 {
                0 => Command::UpdateCar(CarID { 
                    id: i, 
                    vehicle_type: VehicleType::Car 
                }),
                1 => Command::UpdatePed(PedestrianID(i)),
                2 => Command::UpdateIntersection(map_model::IntersectionID(i)),
                _ => Command::UpdateIntersection(map_model::IntersectionID(i + 1000000)),  // Ensure unique IDs
            };
            
            scheduler.push(time, cmd);
        }
        
        // Simulate 20% updates (rescheduling)
        let num_updates = num_operations / 5;
        for i in 0..num_updates {
            let cmd_id = i * 5;
            let new_time = Time::START_OF_DAY + Duration::seconds((cmd_id as f64 / 100.0) + 1.0);
            
            let cmd = match cmd_id % 4 {
                0 => Command::UpdateCar(CarID { 
                    id: cmd_id, 
                    vehicle_type: VehicleType::Car 
                }),
                1 => Command::UpdatePed(PedestrianID(cmd_id)),
                2 => Command::UpdateIntersection(map_model::IntersectionID(cmd_id)),
                _ => Command::UpdateIntersection(map_model::IntersectionID(cmd_id + 1000000)),  // Ensure unique IDs
            };
            
            scheduler.update(new_time, cmd);
        }
        
        // Pop all commands
        let mut count = 0;
        while scheduler.peek_next_time().is_some() {
            if let Some(_) = scheduler.get_next() {
                count += 1;
            }
        }
        
        println!("  Processed {} commands", count);
        
        // Print stats
        let stats = scheduler.describe_stats();
        for stat in stats.iter().take(2) {
            println!("  {}", stat);
        }
        
        start.elapsed()
    }
}
