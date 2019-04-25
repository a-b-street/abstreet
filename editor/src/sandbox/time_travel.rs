use crate::ui::UI;
use abstutil::MultiMap;
use ezgui::EventCtx;
use geom::Duration;
use map_model::{Map, Traversable};
use sim::{CarID, DrawCarInput, DrawPedestrianInput, GetDrawAgents, PedestrianID, TIMESTEP};
use std::collections::BTreeMap;

pub struct TimeTravel {
    // TODO Could be more efficient
    state_per_time: BTreeMap<Duration, StateAtTime>,
    pub current_time: Option<Duration>,
    first_time: Duration,
    last_time: Duration,
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
            state_per_time: BTreeMap::new(),
            current_time: None,
            // TODO Good ol' off-by-ones...
            first_time: Duration::ZERO,
            last_time: Duration::ZERO,
            should_record: false,
        }
    }

    pub fn start(&mut self, time: Duration) {
        assert!(self.current_time.is_none());
        self.should_record = true;
        self.current_time = Some(time);
    }

    pub fn record(&mut self, ui: &UI) {
        if !self.should_record {
            return;
        }
        let map = &ui.state.primary.map;
        let sim = &ui.state.primary.sim;
        let now = sim.time();

        // Record state for this timestep, if needed.
        if now == self.last_time {
            return;
        }

        if now != self.last_time + TIMESTEP {
            // We just loaded a new savestate or something. Clear out our memory.
            self.state_per_time.clear();
            self.first_time = now;
            self.last_time = now;
        }
        self.last_time = now;

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
        self.state_per_time.insert(now, state);
    }

    // Returns true if done.
    pub fn event(&mut self, ctx: &mut EventCtx) -> bool {
        let time = self.current_time.unwrap();
        ctx.input.set_mode_with_prompt(
            "Time Traveler",
            format!("Time Traveler at {}", time),
            &ctx.canvas,
        );
        if time > self.first_time && ctx.input.modal_action("rewind") {
            self.current_time = Some(time - TIMESTEP);
        } else if time < self.last_time && ctx.input.modal_action("forwards") {
            self.current_time = Some(time + TIMESTEP);
        } else if ctx.input.modal_action("quit") {
            self.current_time = None;
            return true;
        }
        false
    }

    fn get_current_state(&self) -> &StateAtTime {
        &self.state_per_time[&self.current_time.unwrap()]
    }
}

impl GetDrawAgents for TimeTravel {
    fn time(&self) -> Duration {
        self.current_time.unwrap()
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
