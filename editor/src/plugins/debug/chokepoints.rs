use crate::objects::{Ctx, DEBUG_EXTRA, ID};
use crate::plugins::{Plugin, PluginCtx};
use counter::Counter;
use ezgui::Color;
use map_model::{IntersectionID, LaneID, PathStep};
use piston::input::Key;
use sim::Sim;
use std::collections::HashSet;

const TOP_N: usize = 10;

pub struct ChokepointsFinder {
    lanes: HashSet<LaneID>,
    intersections: HashSet<IntersectionID>,
}

impl ChokepointsFinder {
    pub fn new(ctx: &mut PluginCtx) -> Option<ChokepointsFinder> {
        if ctx
            .input
            .unimportant_key_pressed(Key::C, DEBUG_EXTRA, "find chokepoints of current sim")
        {
            return Some(find_chokepoints(&ctx.primary.sim));
        }
        None
    }
}

impl Plugin for ChokepointsFinder {
    fn new_event(&mut self, ctx: &mut PluginCtx) -> bool {
        if ctx
            .input
            .key_pressed(Key::Return, "stop showing chokepoints")
        {
            return false;
        }

        true
    }

    fn color_for(&self, obj: ID, ctx: Ctx) -> Option<Color> {
        let color = ctx.cs.get("chokepoint", Color::RED);
        match obj {
            ID::Lane(l) if self.lanes.contains(&l) => Some(color),
            ID::Intersection(i) if self.intersections.contains(&i) => Some(color),
            _ => None,
        }
    }
}

fn find_chokepoints(sim: &Sim) -> ChokepointsFinder {
    let mut count_per_lane: Counter<LaneID, usize> = Counter::new();
    let mut count_per_intersection: Counter<IntersectionID, usize> = Counter::new();

    let active = sim.active_agents();
    info!("Finding chokepoints from {} active agents", active.len());
    for a in active.into_iter() {
        for step in sim.get_path(a).unwrap().get_steps() {
            match step {
                PathStep::Lane(l) | PathStep::ContraflowLane(l) => {
                    count_per_lane.update(vec![*l]);
                }
                PathStep::Turn(t) => {
                    count_per_intersection.update(vec![t.parent]);
                }
            }
        }
    }

    let lanes: HashSet<LaneID> = count_per_lane
        .most_common_ordered()
        .into_iter()
        .take(TOP_N)
        .map(|(l, _)| l)
        .collect();
    let intersections: HashSet<IntersectionID> = count_per_intersection
        .most_common_ordered()
        .into_iter()
        .take(TOP_N)
        .map(|(i, _)| i)
        .collect();
    ChokepointsFinder {
        lanes,
        intersections,
    }
}
