use crate::objects::DrawCtx;
use crate::plugins::sim::des_model;
use crate::plugins::PluginCtx;
use ezgui::{EventLoopMode, GfxCtx};
use map_model::{Map, Traversable};
use sim::{CarID, DrawCarInput, DrawPedestrianInput, GetDrawAgents, PedestrianID, Tick};

enum AutoMode {
    Off,
    Forwards,
    Backwards,
}

pub struct SimpleModelController {
    current_tick: Option<Tick>,
    world: Option<des_model::World>,
    mode: AutoMode,
    show_tooltips: bool,
}

impl SimpleModelController {
    pub fn new() -> SimpleModelController {
        SimpleModelController {
            current_tick: None,
            world: None,
            mode: AutoMode::Off,
            show_tooltips: false,
        }
    }

    pub fn is_active(&self) -> bool {
        self.current_tick.is_some()
    }

    // Don't really need to indicate activeness here.
    pub fn event(&mut self, ctx: &mut PluginCtx) {
        if let Some(tick) = self.current_tick {
            ctx.input.set_mode_with_prompt(
                "Simple Model",
                format!("Simple Model at {}", tick),
                &ctx.canvas,
            );
            match self.mode {
                AutoMode::Off => {
                    if tick != Tick::zero() && ctx.input.modal_action("rewind") {
                        self.current_tick = Some(tick.prev());
                    } else if ctx.input.modal_action("forwards") {
                        self.current_tick = Some(tick.next());
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
                        self.current_tick = Some(tick.next());
                    }
                }
                AutoMode::Backwards => {
                    ctx.hints.mode = EventLoopMode::Animation;
                    if tick == Tick::zero() || ctx.input.modal_action("toggle backwards play") {
                        self.mode = AutoMode::Off;
                    } else if ctx.input.is_update_event() {
                        self.current_tick = Some(tick.prev());
                    }
                }
            }
            if ctx.input.modal_action("quit") {
                self.current_tick = None;
                self.mode = AutoMode::Off;
                self.show_tooltips = false;
            }
            if ctx.input.modal_action("toggle tooltips") {
                self.show_tooltips = !self.show_tooltips;
            }
        } else if ctx.input.action_chosen("start simple model") {
            self.current_tick = Some(Tick::zero());
            if self.world.is_none() {
                self.world = Some(des_model::World::new(&ctx.primary.map));
            }
        }
    }

    pub fn draw(&self, g: &mut GfxCtx, ctx: &DrawCtx) {
        if self.show_tooltips {
            self.world.as_ref().unwrap().draw_tooltips(
                g,
                ctx,
                self.current_tick.unwrap().as_time(),
            );
        }
    }

    fn get_cars(&self, map: &Map) -> Vec<DrawCarInput> {
        self.world
            .as_ref()
            .unwrap()
            .get_draw_cars(self.current_tick.unwrap().as_time(), map)
    }
}

impl GetDrawAgents for SimpleModelController {
    fn tick(&self) -> Tick {
        self.current_tick.unwrap()
    }

    fn get_draw_car(&self, id: CarID, map: &Map) -> Option<DrawCarInput> {
        self.get_cars(map).into_iter().find(|x| x.id == id)
    }

    fn get_draw_ped(&self, _id: PedestrianID, _map: &Map) -> Option<DrawPedestrianInput> {
        None
    }

    fn get_draw_cars(&self, on: Traversable, map: &Map) -> Vec<DrawCarInput> {
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
