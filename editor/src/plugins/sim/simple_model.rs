use crate::plugins::sim::des_model;
use crate::plugins::PluginCtx;
use map_model::{Map, Traversable};
use sim::{CarID, DrawCarInput, DrawPedestrianInput, GetDrawAgents, PedestrianID, Tick};

pub struct SimpleModelController {
    current_tick: Option<Tick>,
}

impl SimpleModelController {
    pub fn new() -> SimpleModelController {
        SimpleModelController { current_tick: None }
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
            if tick != Tick::zero() && ctx.input.modal_action("rewind") {
                self.current_tick = Some(tick.prev());
            } else if ctx.input.modal_action("forwards") {
                self.current_tick = Some(tick.next());
            } else if ctx.input.modal_action("quit") {
                self.current_tick = None;
            }
        } else if ctx.input.action_chosen("start simple model") {
            self.current_tick = Some(Tick::zero());
        }
    }

    fn get_cars(&self, map: &Map) -> Vec<DrawCarInput> {
        des_model::get_state(self.current_tick.unwrap().as_time(), map)
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
