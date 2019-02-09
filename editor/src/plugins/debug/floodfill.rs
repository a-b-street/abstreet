use crate::objects::{DrawCtx, ID};
use crate::plugins::{BlockingPlugin, PluginCtx};
use ezgui::{Color, Key};
use map_model::{LaneID, Map};
use std::collections::{HashSet, VecDeque};

pub struct Floodfiller {
    origin: LaneID,
    visited: HashSet<LaneID>,
    // Order of expansion doesn't really matter, could use other things here
    queue: VecDeque<LaneID>,
}

impl Floodfiller {
    pub fn new(ctx: &mut PluginCtx) -> Option<Floodfiller> {
        if let Some(ID::Lane(id)) = ctx.primary.current_selection {
            if ctx
                .input
                .contextual_action(Key::F, "start floodfilling from this lane")
            {
                let mut queue = VecDeque::new();
                queue.push_back(id);
                return Some(Floodfiller {
                    origin: id,
                    queue,
                    visited: HashSet::new(),
                });
            }
        }
        None
    }
}

impl BlockingPlugin for Floodfiller {
    fn blocking_event(&mut self, ctx: &mut PluginCtx) -> bool {
        ctx.input.set_mode_with_prompt(
            "Floodfiller",
            format!("Floodfiller from {}", self.origin),
            &ctx.canvas,
        );
        if ctx.input.modal_action("quit") {
            return false;
        } else if !self.queue.is_empty() {
            if ctx.input.modal_action("step forwards") {
                step(&mut self.visited, &mut self.queue, &ctx.primary.map);
            }
            if ctx.input.modal_action("finish floodfilling") {
                loop {
                    if step(&mut self.visited, &mut self.queue, &ctx.primary.map) {
                        break;
                    }
                }
            }
        }
        true
    }

    fn color_for(&self, obj: ID, ctx: &DrawCtx) -> Option<Color> {
        if let ID::Lane(l) = obj {
            if self.visited.contains(&l) {
                return Some(ctx.cs.get_def("visited in floodfill", Color::BLUE));
            }
            if !self.queue.is_empty() && *self.queue.front().unwrap() == l {
                return Some(ctx.cs.get_def("next to visit in floodfill", Color::GREEN));
            }
            // TODO linear search shouldnt suck too much for interactive mode
            if self.queue.contains(&l) {
                return Some(ctx.cs.get_def("queued in floodfill", Color::RED));
            }
        }
        None
    }
}

// TODO step backwards!
// TODO doesn't guarantee all visited lanes are connected? are dead-ends possible with the current
// turn definitions?
fn step(visited: &mut HashSet<LaneID>, queue: &mut VecDeque<LaneID>, map: &Map) -> bool {
    loop {
        if queue.is_empty() {
            return true;
        }

        let l = map.get_l(queue.pop_front().unwrap());
        if visited.contains(&l.id) {
            continue;
        }
        visited.insert(l.id);
        for t in &map.get_turns_from_lane(l.id) {
            if !visited.contains(&t.id.dst) {
                queue.push_back(t.id.dst);
            }
        }

        return false;
    }
}
