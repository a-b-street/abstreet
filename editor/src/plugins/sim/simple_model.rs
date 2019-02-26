use crate::objects::{DrawCtx, ID};
use crate::plugins::sim::des_model;
use crate::plugins::{BlockingPlugin, PluginCtx};
use ezgui::{EventLoopMode, GfxCtx, Key};
use geom::{Distance, Duration};
use map_model::{Map, Traversable};
use sim::{CarID, DrawCarInput, DrawPedestrianInput, GetDrawAgents, PedestrianID};

const TIMESTEP: Duration = Duration::const_seconds(0.1);

enum AutoMode {
    Off,
    Forwards,
    Backwards,
}

pub struct SimpleModelController {
    current_time: Duration,
    world: des_model::World,
    mode: AutoMode,
    show_tooltips: bool,
}

impl SimpleModelController {
    pub fn new(ctx: &mut PluginCtx) -> Option<SimpleModelController> {
        if let Some(ID::Lane(id)) = ctx.primary.current_selection {
            if ctx.primary.map.get_l(id).is_driving()
                && ctx.input.contextual_action(Key::C, "start simple model")
            {
                return Some(SimpleModelController {
                    current_time: Duration::ZERO,
                    world: des_model::World::new(id, &ctx.primary.map),
                    mode: AutoMode::Off,
                    show_tooltips: false,
                });
            }
        }
        None
    }

    fn get_cars(&self, map: &Map) -> Vec<DrawCarInput> {
        self.world.get_draw_cars(self.current_time, map)
    }
}

impl BlockingPlugin for SimpleModelController {
    fn blocking_event(&mut self, ctx: &mut PluginCtx) -> bool {
        ctx.input.set_mode_with_prompt(
            "Simple Model",
            format!("Simple Model at {}", self.current_time),
            &ctx.canvas,
        );
        match self.mode {
            AutoMode::Off => {
                if self.current_time != Duration::ZERO && ctx.input.modal_action("rewind") {
                    self.current_time -= TIMESTEP;
                } else if ctx.input.modal_action("forwards") {
                    self.current_time += TIMESTEP;
                } else if ctx.input.modal_action("toggle forwards play") {
                    self.mode = AutoMode::Forwards;
                    ctx.hints.mode = EventLoopMode::Animation;
                } else if ctx.input.modal_action("toggle backwards play") {
                    self.mode = AutoMode::Backwards;
                    ctx.hints.mode = EventLoopMode::Animation;
                }
            }
            AutoMode::Forwards => {
                ctx.hints.mode = EventLoopMode::Animation;
                if ctx.input.modal_action("toggle forwards play") {
                    self.mode = AutoMode::Off;
                } else if ctx.input.is_update_event() {
                    self.current_time += TIMESTEP;
                }
            }
            AutoMode::Backwards => {
                ctx.hints.mode = EventLoopMode::Animation;
                if self.current_time == Duration::ZERO
                    || ctx.input.modal_action("toggle backwards play")
                {
                    self.mode = AutoMode::Off;
                } else if ctx.input.is_update_event() {
                    self.current_time -= TIMESTEP;
                }
            }
        }
        if ctx.input.modal_action("quit") {
            return false;
        }
        if ctx.input.modal_action("toggle tooltips") {
            self.show_tooltips = !self.show_tooltips;
        }
        if ctx.input.modal_action("debug") {
            self.world.dump_debug(self.current_time);
        }
        if ctx
            .input
            .modal_action("exhaustively test instantiation everywhere")
        {
            for l in ctx.primary.map.all_lanes() {
                if l.is_driving() && l.length() >= Distance::meters(15.0) {
                    println!("Testing {}...", l.id);
                    des_model::World::new(l.id, &ctx.primary.map).sample_for_proximity();
                }
            }
        }
        true
    }

    fn draw(&self, g: &mut GfxCtx, ctx: &DrawCtx) {
        if self.show_tooltips {
            self.world.draw_tooltips(g, ctx, self.current_time);
        }
    }
}

impl GetDrawAgents for SimpleModelController {
    fn time(&self) -> Duration {
        self.current_time
    }

    fn get_draw_car(&self, id: CarID, map: &Map) -> Option<DrawCarInput> {
        self.get_cars(map).into_iter().find(|x| x.id == id)
    }

    fn get_draw_ped(&self, _id: PedestrianID, _map: &Map) -> Option<DrawPedestrianInput> {
        None
    }

    fn get_draw_cars(&self, on: Traversable, map: &Map) -> Vec<DrawCarInput> {
        if on != Traversable::Lane(self.world.lane) {
            return Vec::new();
        }

        self.get_cars(map)
            .into_iter()
            .filter(|x| x.on == on)
            .collect()
    }

    fn get_draw_peds(&self, _on: Traversable, _map: &Map) -> Vec<DrawPedestrianInput> {
        Vec::new()
    }

    fn get_all_draw_cars(&self, map: &Map) -> Vec<DrawCarInput> {
        self.get_cars(map)
    }

    fn get_all_draw_peds(&self, _map: &Map) -> Vec<DrawPedestrianInput> {
        Vec::new()
    }
}
