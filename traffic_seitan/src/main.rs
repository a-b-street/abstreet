// This is a tool that runs a simulation, constantly interrupting to apply random map edits to the
// live sim without resetting to midnight. The purpose is to trigger crashes and find bugs.
//
// TODO Eventually rewrite this to go through the public API. Faster to iterate in Rust for now.

#[macro_use]
extern crate log;

use rand::seq::SliceRandom;
use rand_xorshift::XorShiftRng;

use abstutil::{prettyprint_usize, CmdArgs, Timer};
use geom::Duration;
use map_model::{LaneID, LaneType, Map, MapEdits};
use sim::{Sim, SimFlags};

fn main() {
    let mut args = CmdArgs::new();
    let sim_flags = SimFlags::from_args(&mut args);
    args.done();

    let mut timer = Timer::throwaway();
    let (mut map, mut sim, mut rng) = sim_flags.load_synchronously(&mut timer);

    // Set the edits name up-front, so that the savestates get named reasonably too.
    {
        let mut edits = map.get_edits().clone();
        edits.edits_name = "traffic_seitan".to_string();
        map.must_apply_edits(edits);
        map.recalculate_pathfinding_after_edits(&mut timer);
        sim.handle_live_edits(&map);
    }

    if let Err(err) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run(&mut map, &mut sim, &mut rng, &mut timer);
    })) {
        let mut edits = map.get_edits().clone();
        edits.edits_name = "traffic_seitan_crash".to_string();
        map.must_apply_edits(edits);
        map.save_edits();

        println!("Crashed at {}", sim.time());

        std::panic::resume_unwind(err)
    }
}

fn run(map: &mut Map, sim: &mut Sim, rng: &mut XorShiftRng, timer: &mut Timer) {
    let edit_frequency = Duration::minutes(5);

    while !sim.is_done() {
        println!();
        sim.timed_step(map, edit_frequency, &mut None, timer);
        sim.save();
        map.save_edits();

        let mut edits = map.get_edits().clone();
        nuke_random_parking(map, rng, &mut edits);
        alter_turn_destinations(sim, map, rng, &mut edits);

        map.must_apply_edits(edits);
        map.recalculate_pathfinding_after_edits(timer);
        sim.handle_live_edited_traffic_signals(map);
        sim.handle_live_edits(map);
    }

    let mut finished = 0;
    let mut cancelled = 0;
    for (_, _, _, maybe_dt) in &sim.get_analytics().finished_trips {
        if maybe_dt.is_some() {
            finished += 1;
        } else {
            cancelled += 1;
        }
    }
    println!(
        "\nDone! {} finished trips, {} cancelled",
        prettyprint_usize(finished),
        prettyprint_usize(cancelled)
    );
}

fn alter_turn_destinations(sim: &Sim, map: &Map, rng: &mut XorShiftRng, edits: &mut MapEdits) {
    let num_edits = 3;

    // Find active turns
    let mut active_destinations = Vec::new();
    for i in map.all_intersections() {
        for (_, t) in sim.get_accepted_agents(i.id) {
            if !map.get_l(t.dst).is_walkable() {
                active_destinations.push(t.dst);
            }
        }
    }
    active_destinations.sort();
    active_destinations.dedup();
    active_destinations.shuffle(rng);

    for l in active_destinations.into_iter().take(num_edits) {
        info!("Closing someone's target {}", l);
        let r = map.get_parent(l);
        edits.commands.push(map.edit_road_cmd(r.id, |new| {
            new.lanes_ltr[r.offset(l)].lt = LaneType::Construction;

            // If we're getting rid of the last driving lane, also remove any parking lanes. This
            // mimics the check that the UI does.
            if new
                .lanes_ltr
                .iter()
                .all(|spec| spec.lt != LaneType::Driving)
            {
                for spec in &mut new.lanes_ltr {
                    if spec.lt == LaneType::Parking {
                        spec.lt = LaneType::Construction;
                    }
                }
            }
        }));
    }
}

fn nuke_random_parking(map: &Map, rng: &mut XorShiftRng, edits: &mut MapEdits) {
    let num_edits = 5;

    let mut parking_lanes: Vec<LaneID> = map
        .all_lanes()
        .values()
        .filter(|l| l.is_parking())
        .map(|l| l.id)
        .collect();
    parking_lanes.shuffle(rng);
    for l in parking_lanes.into_iter().take(num_edits) {
        info!("Closing parking {}", l);
        let r = map.get_parent(l);
        edits.commands.push(map.edit_road_cmd(r.id, |new| {
            new.lanes_ltr[r.offset(l)].lt = LaneType::Construction;
        }));
    }
}
