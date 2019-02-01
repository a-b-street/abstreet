use crate::plugins::PluginCtx;
use abstutil::MultiMap;
use map_model::{Map, Traversable};
use sim::{CarID, DrawCarInput, DrawPedestrianInput, GetDrawAgents, PedestrianID, Sim, Tick};
use std::collections::BTreeMap;

pub struct TimeTravel {
    state_per_tick: Vec<StateAtTime>,
    current_tick: Option<Tick>,
    // Determines the tick of state_per_tick[0]
    first_tick: Tick,
    should_record: bool,
}

struct StateAtTime {
    cars: BTreeMap<CarID, DrawCarInput>,
    peds: BTreeMap<PedestrianID, DrawPedestrianInput>,
    cars_per_traversable: MultiMap<Traversable, CarID>,
    peds_per_traversable: MultiMap<Traversable, PedestrianID>,
}

impl TimeTravel {
    pub fn new() -> TimeTravel {
        TimeTravel {
            state_per_tick: Vec::new(),
            current_tick: None,
            first_tick: Tick::zero(),
            should_record: false,
        }
    }

    pub fn is_active(&self) -> bool {
        self.current_tick.is_some()
    }

    fn record_state(&mut self, sim: &Sim, map: &Map) {
        // Record state for this tick, if needed.
        let tick = sim.time.as_usize();
        if tick + 1 == self.first_tick.as_usize() + self.state_per_tick.len() {
            return;
        }
        if tick != self.first_tick.as_usize() + self.state_per_tick.len() {
            // We just loaded a new savestate or something. Clear out our memory.
            self.state_per_tick.clear();
            self.first_tick = sim.time;
        }

        let mut state = StateAtTime {
            cars: BTreeMap::new(),
            peds: BTreeMap::new(),
            cars_per_traversable: MultiMap::new(),
            peds_per_traversable: MultiMap::new(),
        };
        for draw in sim.get_all_draw_cars(map).into_iter() {
            state.cars_per_traversable.insert(draw.on, draw.id);
            state.cars.insert(draw.id, draw);
        }
        for draw in sim.get_all_draw_peds(map).into_iter() {
            state.peds_per_traversable.insert(draw.on, draw.id);
            state.peds.insert(draw.id, draw);
        }
        self.state_per_tick.push(state);
    }

    fn get_current_state(&self) -> &StateAtTime {
        &self.state_per_tick[self.current_tick.unwrap().as_usize() - self.first_tick.as_usize()]
    }

    // Don't really need to indicate activeness here.
    pub fn event(&mut self, ctx: &mut PluginCtx) {
        if self.should_record {
            self.record_state(&ctx.primary.sim, &ctx.primary.map);
        }

        if let Some(tick) = self.current_tick {
            ctx.input.set_mode_with_prompt(
                "Time Traveler",
                format!("Time Traveler at {}", tick),
                &ctx.canvas,
            );
            if tick > self.first_tick && ctx.input.modal_action("rewind") {
                self.current_tick = Some(tick.prev());
            } else if tick.as_usize() + 1 < self.first_tick.as_usize() + self.state_per_tick.len()
                && ctx.input.modal_action("forwards")
            {
                self.current_tick = Some(tick.next());
            } else if ctx.input.modal_action("quit") {
                self.current_tick = None;
            }
        } else if ctx.input.action_chosen("start time traveling") {
            if !self.should_record {
                self.should_record = true;
                self.record_state(&ctx.primary.sim, &ctx.primary.map);
            }
            self.current_tick = Some(ctx.primary.sim.time);
        }
    }
}

impl GetDrawAgents for TimeTravel {
    fn tick(&self) -> Tick {
        self.current_tick.unwrap()
    }

    fn get_draw_car(&self, id: CarID, _map: &Map) -> Option<DrawCarInput> {
        self.get_current_state().cars.get(&id).cloned()
    }

    fn get_draw_ped(&self, id: PedestrianID, _map: &Map) -> Option<DrawPedestrianInput> {
        self.get_current_state().peds.get(&id).cloned()
    }

    fn get_draw_cars(&self, on: Traversable, _map: &Map) -> Vec<DrawCarInput> {
        let state = self.get_current_state();
        // TODO sort by ID to be deterministic?
        state
            .cars_per_traversable
            .get(on)
            .into_iter()
            .map(|id| state.cars[id].clone())
            .collect()
    }

    fn get_draw_peds(&self, on: Traversable, _map: &Map) -> Vec<DrawPedestrianInput> {
        let state = self.get_current_state();
        state
            .peds_per_traversable
            .get(on)
            .into_iter()
            .map(|id| state.peds[id].clone())
            .collect()
    }

    fn get_all_draw_cars(&self, _map: &Map) -> Vec<DrawCarInput> {
        self.get_current_state().cars.values().cloned().collect()
    }

    fn get_all_draw_peds(&self, _map: &Map) -> Vec<DrawPedestrianInput> {
        self.get_current_state().peds.values().cloned().collect()
    }
}
