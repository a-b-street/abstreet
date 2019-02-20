use crate::objects::{DrawCtx, ID};
use crate::plugins::sim::new_des_model;
use crate::plugins::{BlockingPlugin, PluginCtx};
use ezgui::{EventLoopMode, GfxCtx, Key};
use geom::Duration;
use map_model::{LaneID, Map, Traversable};
use rand::seq::SliceRandom;
use rand::SeedableRng;
use rand_xorshift::XorShiftRng;
use sim::{CarID, VehicleType};

const TIMESTEP: Duration = Duration::const_seconds(0.1);

pub struct EvenSimplerModelController {
    current_time: Duration,
    world: new_des_model::World,
    auto_mode: bool,
}

impl EvenSimplerModelController {
    pub fn new(ctx: &mut PluginCtx) -> Option<EvenSimplerModelController> {
        if let Some(ID::Lane(id)) = ctx.primary.current_selection {
            if ctx.primary.map.get_l(id).is_driving()
                && ctx
                    .input
                    .contextual_action(Key::Z, "start even simpler model")
            {
                return Some(EvenSimplerModelController {
                    current_time: Duration::ZERO,
                    world: populate_world(id, &ctx.primary.map),
                    auto_mode: false,
                });
            }
        }
        None
    }
}

impl BlockingPlugin for EvenSimplerModelController {
    fn blocking_event(&mut self, ctx: &mut PluginCtx) -> bool {
        ctx.input.set_mode_with_prompt(
            "Even Simpler Model",
            format!("Even Simpler Model at {}", self.current_time),
            &ctx.canvas,
        );
        if self.auto_mode {
            ctx.hints.mode = EventLoopMode::Animation;
            if ctx.input.modal_action("toggle forwards play") {
                self.auto_mode = false;
            } else if ctx.input.is_update_event() {
                self.current_time += TIMESTEP;
                self.world
                    .step_if_needed(self.current_time, &ctx.primary.map);
            }
        } else {
            if ctx.input.modal_action("forwards") {
                self.current_time += TIMESTEP;
                self.world
                    .step_if_needed(self.current_time, &ctx.primary.map);
            } else if ctx.input.modal_action("toggle forwards play") {
                self.auto_mode = true;
                ctx.hints.mode = EventLoopMode::Animation;
            }
        }
        if ctx.input.modal_action("quit") {
            return false;
        }
        true
    }

    fn draw(&self, g: &mut GfxCtx, ctx: &DrawCtx) {
        self.world.draw_unzoomed(self.current_time, g, &ctx.map);
    }
}

fn populate_world(start: LaneID, map: &Map) -> new_des_model::World {
    let mut world = new_des_model::World::new(map);

    let mut sources = vec![start];
    // Try to find a lane likely to have conflicts
    {
        for t in map.get_turns_from_lane(start) {
            if t.turn_type == map_model::TurnType::Straight {
                if let Some(l) = map
                    .get_parent(t.id.dst)
                    .any_on_other_side(t.id.dst, map_model::LaneType::Driving)
                {
                    sources.push(l);
                }
            }
        }
    }

    let mut rng = XorShiftRng::from_seed([42; 16]);
    for source in sources {
        for i in 0..10 {
            let mut path = vec![Traversable::Lane(source)];
            let mut last_lane = source;
            for _ in 0..5 {
                let t = *map.get_turns_from_lane(last_lane).choose(&mut rng).unwrap();
                path.push(Traversable::Turn(t.id));
                path.push(Traversable::Lane(t.id.dst));
                last_lane = t.id.dst;
            }

            world.spawn_car(
                CarID::tmp_new(i, VehicleType::Car),
                None,
                path.clone(),
                Duration::seconds(1.0) * (i as f64),
            );
        }
    }

    world
}
