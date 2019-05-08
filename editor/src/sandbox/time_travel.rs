use crate::ui::UI;
use abstutil::MultiMap;
use ezgui::{Canvas, EventCtx, GfxCtx, Key, ModalMenu, Text};
use geom::Duration;
use map_model::{Map, Traversable};
use sim::{CarID, DrawCarInput, DrawPedestrianInput, GetDrawAgents, PedestrianID};
use std::collections::BTreeMap;

pub struct TimeTravel {
    menu: ModalMenu,
    state_per_time: Vec<StateAtTime>,
    current_idx: Option<usize>,
    should_record: bool,
}

struct StateAtTime {
    time: Duration,
    cars: BTreeMap<CarID, DrawCarInput>,
    peds: BTreeMap<PedestrianID, DrawPedestrianInput>,
    cars_per_traversable: MultiMap<Traversable, CarID>,
    peds_per_traversable: MultiMap<Traversable, PedestrianID>,
}

impl TimeTravel {
    pub fn new(canvas: &Canvas) -> TimeTravel {
        TimeTravel {
            state_per_time: Vec::new(),
            current_idx: None,
            should_record: false,
            menu: ModalMenu::hacky_new(
                "Time Traveler",
                vec![
                    (Some(Key::Escape), "quit"),
                    (Some(Key::Comma), "rewind"),
                    (Some(Key::Dot), "forwards"),
                ],
                canvas,
            ),
        }
    }

    pub fn start(&mut self, ui: &UI) {
        assert!(self.current_idx.is_none());
        self.should_record = true;
        // In case we weren't already...
        self.record(ui);
        self.current_idx = Some(self.state_per_time.len() - 1);
    }

    // TODO Now that we take big jumps forward in the source sim, the time traveler sees the same
    // granularity when replaying.
    pub fn record(&mut self, ui: &UI) {
        if !self.should_record {
            return;
        }
        let map = &ui.primary.map;
        let sim = &ui.primary.sim;
        let now = sim.time();

        if let Some(ref state) = self.state_per_time.last() {
            // Already have this
            if now == state.time {
                return;
            }
            // We just loaded a new savestate or reset or something. Clear out our memory.
            if now < state.time {
                self.state_per_time.clear();
                if self.current_idx.is_some() {
                    self.current_idx = Some(0);
                }
            }
        }

        let mut state = StateAtTime {
            time: now,
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
        self.state_per_time.push(state);
    }

    // Returns true if done.
    pub fn event(&mut self, ctx: &mut EventCtx) -> bool {
        let idx = self.current_idx.unwrap();
        self.menu.handle_event(
            ctx,
            Some(Text::prompt(&format!("Time Traveler at {}", self.time()))),
        );

        ctx.canvas.handle_event(ctx.input);

        if idx > 0 && self.menu.action("rewind") {
            self.current_idx = Some(idx - 1);
        } else if idx != self.state_per_time.len() - 1 && self.menu.action("forwards") {
            self.current_idx = Some(idx + 1);
        } else if self.menu.action("quit") {
            self.current_idx = None;
            return true;
        }
        false
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        self.menu.draw(g);
    }

    fn get_current_state(&self) -> &StateAtTime {
        &self.state_per_time[self.current_idx.unwrap()]
    }
}

impl GetDrawAgents for TimeTravel {
    fn time(&self) -> Duration {
        self.state_per_time[self.current_idx.unwrap()].time
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
