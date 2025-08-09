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
    
    // Optimization: track stale entries for periodic cleanup
    #[serde(skip_serializing, skip_deserializing)]
    stale_count: usize,
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
            stale_count: 0,
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
            // The old entry in the heap becomes stale
            self.stale_count += 1;
        }
        self.queued_commands
            .insert(cmd_type.clone(), (cmd, new_time));
        self.items.push(PriorityQueueItem {
            cost: new_time,
            value: cmd_type,
        });
    }

    pub fn cancel(&mut self, cmd: Command) {
        // It's fine if a previous command hasn't actually been scheduled.
        if self.queued_commands.remove(&cmd.to_type()).is_some() {
            // The entry in the heap becomes stale
            self.stale_count += 1;
        }
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
        // Clean up if we have too many stale entries relative to total heap size
        if self.stale_count > 1000 && self.stale_count > self.items.len() / 3 {
            self.cleanup_stale_entries();
        }
        
        loop {
            let item = self.items.pop()?;
            match self.queued_commands.entry(item.value) {
                Entry::Vacant(_) => {
                    // Command was cancelled - this was a stale entry
                    self.stale_count = self.stale_count.saturating_sub(1);
                    continue;
                }
                Entry::Occupied(occupied) => {
                    // Command was re-scheduled for later.
                    if occupied.get().1 > item.cost {
                        // This was a stale entry
                        self.stale_count = self.stale_count.saturating_sub(1);
                        continue;
                    }
                    // Valid command found
                    self.latest_time = item.cost;
                    return Some(occupied.remove().0);
                }
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
    
    /// Remove stale entries from the heap when they build up too much
    fn cleanup_stale_entries(&mut self) {
        let old_size = self.items.len();
        
        // Rebuild heap with only valid entries
        let valid_items: Vec<_> = self.items
            .drain()
            .filter(|item| {
                self.queued_commands.get(&item.value)
                    .map(|(_, time)| *time == item.cost)
                    .unwrap_or(false)
            })
            .collect();
        
        self.items = BinaryHeap::from(valid_items);
        let new_size = self.items.len();
        self.stale_count = 0;
        
        // Optional: log significant cleanups for debugging
        if old_size > new_size + 100 {
            log::debug!("Scheduler cleanup: removed {} stale entries ({} -> {})", 
                       old_size - new_size, old_size, new_size);
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
        
        // Test with different sizes to verify no performance regression
        let test_sizes = vec![10_000, 50_000, 100_000];
        
        for size in test_sizes {
            println!("\nTesting with {} operations:", size);
            let duration = benchmark_scheduler(size);
            println!("  Time: {:?}", duration);
            
            // Should be reasonably fast - if it takes more than 1 second for 100k ops, something's wrong
            if size == 100_000 {
                assert!(duration.as_millis() < 1000, "Performance regression: took {:?} for {}k operations", duration, size / 1000);
            }
        }
    }

    #[test]
    fn test_scheduler_performance_with_heavy_updates() {
        println!("\nTesting scheduler with heavy update pattern (many stale entries)...");
        
        let start = Instant::now();
        let mut scheduler = Scheduler::new();
        
        // Create 50k initial commands
        for i in 0..50_000 {
            let time = Time::START_OF_DAY + Duration::seconds(i as f64 / 100.0);
            let cmd = Command::UpdateCar(CarID { 
                id: i, 
                vehicle_type: VehicleType::Car 
            });
            scheduler.push(time, cmd);
        }
        
        let after_push = start.elapsed();
        
        // Update 80% of them (creates lots of stale entries)
        for i in 0..40_000 {
            let new_time = Time::START_OF_DAY + Duration::seconds(i as f64 / 100.0 + 100.0);
            let cmd = Command::UpdateCar(CarID { 
                id: i, 
                vehicle_type: VehicleType::Car 
            });
            scheduler.update(new_time, cmd);
        }
        
        let after_updates = start.elapsed();
        
        // Cancel 20% more (creates even more stale entries)
        for i in 0..10_000 {
            let cmd = Command::UpdateCar(CarID { 
                id: i + 40_000, 
                vehicle_type: VehicleType::Car 
            });
            scheduler.cancel(cmd);
        }
        
        let after_cancels = start.elapsed();
        
        // Now process all remaining commands
        let mut processed = 0;
        while scheduler.peek_next_time().is_some() {
            if scheduler.get_next().is_some() {
                processed += 1;
            }
        }
        
        let total_time = start.elapsed();
        
        println!("  Push phase: {:?}", after_push);
        println!("  Update phase: {:?}", after_updates - after_push);
        println!("  Cancel phase: {:?}", after_cancels - after_updates);
        println!("  Process phase: {:?}", total_time - after_cancels);
        println!("  Total time: {:?}", total_time);
        println!("  Processed {} commands (expected ~40k)", processed);
        
        // With optimization, this should complete reasonably fast despite heavy churn
        assert!(total_time.as_millis() < 2000, "Too slow with heavy updates: {:?}", total_time);
        assert_eq!(processed, 40_000); // 50k - 10k cancelled = 40k remaining
        
        // This test demonstrates the optimization's benefit:
        // - Creates 50k entries, then 40k updates (40k stale entries in heap)  
        // - Plus 10k cancellations (10k more stale entries)
        // - So heap has ~90k entries but only 40k are valid
        // - Without optimization: would scan through many stale entries
        // - With optimization: cleanup triggers and removes stale entries efficiently
    }

    #[test]
    fn test_stale_entry_tracking() {
        let mut scheduler = Scheduler::new();
        
        // Add initial commands
        for i in 0..10 {
            let cmd = Command::UpdateCar(CarID { id: i, vehicle_type: VehicleType::Car });
            scheduler.push(Time::START_OF_DAY + Duration::seconds(i as f64), cmd);
        }
        assert_eq!(scheduler.stale_count, 0);
        
        // Update some commands (creates stale entries)
        for i in 0..5 {
            let cmd = Command::UpdateCar(CarID { id: i, vehicle_type: VehicleType::Car });
            scheduler.update(Time::START_OF_DAY + Duration::seconds(i as f64 + 10.0), cmd);
        }
        assert_eq!(scheduler.stale_count, 5);
        
        // Cancel some commands (creates more stale entries)
        for i in 5..8 {
            let cmd = Command::UpdateCar(CarID { id: i, vehicle_type: VehicleType::Car });
            scheduler.cancel(cmd);
        }
        assert_eq!(scheduler.stale_count, 8);
    }

    #[test]
    fn test_cleanup_doesnt_trigger_prematurely() {
        let mut scheduler = Scheduler::new();
        
        // Add commands but stay below cleanup thresholds
        for i in 0..500 {
            let cmd = Command::UpdatePed(PedestrianID(i));
            scheduler.push(Time::START_OF_DAY + Duration::seconds(i as f64), cmd);
        }
        
        // Update half (creates 250 stale entries, but < 1000 threshold)
        for i in 0..250 {
            let cmd = Command::UpdatePed(PedestrianID(i));
            scheduler.update(Time::START_OF_DAY + Duration::seconds(i as f64 + 100.0), cmd);
        }
        
        // Heap should still have both old and new entries
        assert!(scheduler.items.len() > 500); // 500 original + 250 updates = 750+
        assert_eq!(scheduler.stale_count, 250);
        
        // Process one command - should not trigger cleanup
        if let Some(_) = scheduler.get_next() {
            // Stale count should only decrease if we hit a stale entry
            assert!(scheduler.stale_count <= 250);
        }
    }

    #[test]
    fn test_cleanup_triggers_when_needed() {
        let mut scheduler = Scheduler::new();
        
        // Create enough commands to trigger cleanup
        for i in 0..2000 {
            let cmd = Command::UpdateIntersection(map_model::IntersectionID(i));
            scheduler.push(Time::START_OF_DAY + Duration::seconds(i as f64), cmd);
        }
        
        // Update most of them to create > 1000 stale entries
        for i in 0..1500 {
            let cmd = Command::UpdateIntersection(map_model::IntersectionID(i));
            scheduler.update(Time::START_OF_DAY + Duration::seconds(i as f64 + 100.0), cmd);
        }
        
        let heap_size_before = scheduler.items.len();
        let stale_count_before = scheduler.stale_count;
        
        assert!(stale_count_before >= 1500);
        assert!(heap_size_before >= 3000); // 2000 + 1500 updates
        
        // Process one command - should trigger cleanup
        scheduler.get_next();
        
        // After cleanup, heap should be smaller and stale count reset
        assert!(scheduler.items.len() < heap_size_before);
        assert_eq!(scheduler.stale_count, 0);
    }

    #[test]
    fn test_get_next_skips_stale_entries_correctly() {
        let mut scheduler = Scheduler::new();
        
        // Add commands at different times
        for i in 0..10 {
            let cmd = Command::UpdateCar(CarID { id: i, vehicle_type: VehicleType::Car });
            scheduler.push(Time::START_OF_DAY + Duration::seconds(i as f64), cmd);
        }
        
        // Cancel every other command
        for i in (0..10).step_by(2) {
            let cmd = Command::UpdateCar(CarID { id: i, vehicle_type: VehicleType::Car });
            scheduler.cancel(cmd);
        }
        
        // Should only get the non-cancelled commands (1, 3, 5, 7, 9)
        let mut received_ids = Vec::new();
        while scheduler.peek_next_time().is_some() {
            if let Some(cmd) = scheduler.get_next() {
                if let Command::UpdateCar(car_id) = cmd {
                    received_ids.push(car_id.id);
                }
            }
        }
        
        received_ids.sort();
        assert_eq!(received_ids, vec![1, 3, 5, 7, 9]);
    }

    #[test]
    fn test_reschedule_creates_correct_stale_entries() {
        let mut scheduler = Scheduler::new();
        
        // Add command at time 1
        let cmd = Command::UpdatePed(PedestrianID(1));
        scheduler.push(Time::START_OF_DAY + Duration::seconds(1.0), cmd.clone());
        assert_eq!(scheduler.stale_count, 0);
        
        // Reschedule to time 10 (old entry becomes stale)
        scheduler.update(Time::START_OF_DAY + Duration::seconds(10.0), cmd.clone());
        assert_eq!(scheduler.stale_count, 1);
        
        // Reschedule again to time 20 (previous entry becomes stale)
        scheduler.update(Time::START_OF_DAY + Duration::seconds(20.0), cmd);
        assert_eq!(scheduler.stale_count, 2);
        
        // Should have 3 entries in heap but only 1 valid
        assert_eq!(scheduler.items.len(), 3);
        assert_eq!(scheduler.queued_commands.len(), 1);
        
        // Get the command - should get the latest one (time 20)
        if let Some(cmd) = scheduler.get_next() {
            assert_eq!(cmd, Command::UpdatePed(PedestrianID(1)));
            assert_eq!(scheduler.latest_time, Time::START_OF_DAY + Duration::seconds(20.0));
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
                _ => Command::UpdateIntersection(map_model::IntersectionID(i + 1000000)),
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
                _ => Command::UpdateIntersection(map_model::IntersectionID(cmd_id + 1000000)),
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
