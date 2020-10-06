// This is a tool that runs a simulation, constantly interrupting to apply random map edits to the
// live sim without resetting to midnight. The purpose is to trigger crashes and find bugs.
//
// TODO When something does crash, we'll need to repro in the GUI. Savestate and give instructions
// how to make the last edit at a time?
//
// TODO Eventually rewrite this to go through the public API. Faster to iterate in Rust for now.

use rand::seq::SliceRandom;
use rand_xorshift::XorShiftRng;

use abstutil::{CmdArgs, Timer};
use geom::Duration;
use map_model::{LaneID, LaneType, Map, MapEdits};
use sim::{Sim, SimFlags};

fn main() {
    let mut args = CmdArgs::new();
    let sim_flags = SimFlags::from_args(&mut args);
    args.done();

    let mut timer = Timer::new("cause mass chaos");
    let (mut map, mut sim, mut rng) = sim_flags.load(&mut timer);

    if let Err(err) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run(&mut map, &mut sim, &mut rng, &mut timer);
    })) {
        let mut edits = map.get_edits().clone();
        edits.edits_name = "traffic_seitan_crash".to_string();
        map.must_apply_edits(edits, &mut timer);
        map.save_edits();

        std::panic::resume_unwind(err)
    }
}

fn run(map: &mut Map, sim: &mut Sim, rng: &mut XorShiftRng, timer: &mut Timer) {
    let edit_frequency = Duration::minutes(5);

    while !sim.is_done() {
        sim.timed_step(map, edit_frequency, &mut None, timer);

        let mut edits = map.get_edits().clone();
        edits.edits_name = "chaos".to_string();
        nuke_random_parking(map, rng, &mut edits);
        alter_turn_destinations(sim, map, rng, &mut edits);
        map.must_apply_edits(edits, timer);
        map.recalculate_pathfinding_after_edits(timer);
    }
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
        // TODO Also need to change all parking lanes on the road; otherwise we might wind up
        // making an edit that the UI blocks the player from doing.
        let r = map.get_parent(l);
        edits.commands.push(map.edit_road_cmd(r.id, |new| {
            new.lanes_ltr[r.offset(l)].0 = LaneType::Construction;
        }));
    }
}

// TODO This doesn't cause any interesting crash yet. Find somebody in the act of
// parking/unparking/going to a spot, and nuke that instead.
fn nuke_random_parking(map: &Map, rng: &mut XorShiftRng, edits: &mut MapEdits) {
    let num_edits = 5;

    let mut parking_lanes: Vec<LaneID> = map
        .all_lanes()
        .iter()
        .filter(|l| l.is_parking())
        .map(|l| l.id)
        .collect();
    parking_lanes.shuffle(rng);
    for l in parking_lanes.into_iter().take(num_edits) {
        let r = map.get_parent(l);
        edits.commands.push(map.edit_road_cmd(r.id, |new| {
            new.lanes_ltr[r.offset(l)].0 = LaneType::Construction;
        }));
    }
}
