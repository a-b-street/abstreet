use super::*;
use crate::{CarID, PedestrianID, VehicleType};
use geom::{Duration, Time};
use std::time::Instant;

#[test]
fn test_cleanup_triggers_after_operations() {
    let mut scheduler = Scheduler::new();
    
    // Add many operations to trigger cleanup
    for i in 0..12000 {
        let cmd = Command::UpdateCar(CarID { id: i, vehicle_type: VehicleType::Car });
        scheduler.push(Time::START_OF_DAY + Duration::seconds(i as f64), cmd);
    }
    
    let stats = scheduler.describe_stats();
    println!("After 12k operations: {}", stats[1]);
    
    // Should have triggered cleanup (operations > 10k threshold)
    assert!(stats[1].contains("heap size"));
}

#[test] 
fn test_stale_entry_counting() {
    let mut scheduler = Scheduler::new();
    
    // Add initial commands
    for i in 0..100 {
        let cmd = Command::UpdateCar(CarID { id: i, vehicle_type: VehicleType::Car });
        scheduler.push(Time::START_OF_DAY + Duration::seconds(i as f64), cmd);
    }
    
    // Create stale entries by updating
    for i in 0..50 {
        let cmd = Command::UpdateCar(CarID { id: i, vehicle_type: VehicleType::Car });
        scheduler.update(Time::START_OF_DAY + Duration::seconds(i as f64 + 10.0), cmd);
    }
    
    let stats = scheduler.describe_stats();
    println!("After updates: {}", stats[1]);
    
    // Should track stale entries
    assert!(stats[1].contains("stale entries"));
}

#[test]
fn test_cancel_tracking() {
    let mut scheduler = Scheduler::new();
    
    // Add commands
    for i in 0..20 {
        let cmd = Command::UpdatePed(PedestrianID(i));
        scheduler.push(Time::START_OF_DAY + Duration::seconds(i as f64), cmd);
    }
    
    // Cancel some
    for i in 0..10 {
        let cmd = Command::UpdatePed(PedestrianID(i));
        scheduler.cancel(cmd);
    }
    
    let stats = scheduler.describe_stats();
    println!("After cancels: {}", stats[1]);
    
    // Should have correct counts
    let mut remaining = 0;
    while scheduler.peek_next_time().is_some() {
        if scheduler.get_next().is_some() {
            remaining += 1;
        }
    }
    
    assert_eq!(remaining, 10); // Should have 10 remaining commands
}

#[test]
fn test_performance_with_many_updates() {
    println!("\n=== Performance Test with Heavy Updates ===");
    
    let start = Instant::now();
    let mut scheduler = Scheduler::new();
    
    // Add base commands
    for i in 0..10000 {
        let cmd = Command::UpdateCar(CarID { id: i, vehicle_type: VehicleType::Car });
        scheduler.push(Time::START_OF_DAY + Duration::seconds(i as f64), cmd);
    }
    
    let after_push = start.elapsed();
    
    // Heavy update pattern (90% of commands get updated)
    for i in 0..9000 {
        let cmd = Command::UpdateCar(CarID { id: i, vehicle_type: VehicleType::Car });
        scheduler.update(Time::START_OF_DAY + Duration::seconds(i as f64 + 100.0), cmd);
    }
    
    let after_updates = start.elapsed();
    
    // Process all
    let mut count = 0;
    while scheduler.peek_next_time().is_some() {
        if scheduler.get_next().is_some() {
            count += 1;
        }
    }
    
    let total_time = start.elapsed();
    let final_stats = scheduler.describe_stats();
    
    println!("Push time: {:?}", after_push);
    println!("Update time: {:?}", after_updates - after_push);
    println!("Process time: {:?}", total_time - after_updates);
    println!("Total time: {:?}", total_time);
    println!("Processed: {} commands", count);
    println!("Final stats: {}", final_stats[1]);
    
    // Should complete in reasonable time
    assert!(total_time.as_millis() < 1000);
    assert_eq!(count, 10000);
}

#[test]
fn test_memory_efficiency() {
    let mut scheduler = Scheduler::new();
    
    // Create scenario that would cause memory bloat without cleanup
    for round in 0..5 {
        // Add 2000 commands per round
        for i in 0..2000 {
            let id = round * 2000 + i;
            let cmd = Command::UpdateIntersection(map_model::IntersectionID(id));
            scheduler.push(Time::START_OF_DAY + Duration::seconds(id as f64), cmd);
        }
        
        // Update 90% of them (creates stale entries)
        for i in 0..1800 {
            let id = round * 2000 + i;
            let cmd = Command::UpdateIntersection(map_model::IntersectionID(id));
            scheduler.update(Time::START_OF_DAY + Duration::seconds(id as f64 + 50.0), cmd);
        }
    }
    
    let stats = scheduler.describe_stats();
    println!("After memory test: {}", stats[1]);
    
    // Extract heap size
    let heap_size_str = stats[1].split("heap size: ").nth(1).unwrap().split(",").next().unwrap();
    let heap_size: usize = heap_size_str.parse().unwrap();
    
    println!("Final heap size: {}", heap_size);
    
    // Without cleanup, this would be ~19,000 entries (10k base + 9k updates)
    // With cleanup, should be much smaller
    assert!(heap_size < 15000, "Heap too large: {}", heap_size);
}