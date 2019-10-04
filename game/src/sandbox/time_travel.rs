use crate::game::{State, Transition};
use crate::render::DrawOptions;
use crate::sandbox::SandboxMode;
use crate::ui::{ShowEverything, UI};
use abstutil::MultiMap;
use ezgui::{hotkey, EventCtx, GfxCtx, ItemSlider, Key, Line, Text};
use geom::Duration;
use map_model::{Map, Traversable};
use sim::{
    CarID, DrawCarInput, DrawPedCrowdInput, DrawPedestrianInput, GetDrawAgents, PedestrianID,
    UnzoomedAgent,
};
use std::collections::BTreeMap;

pub struct InactiveTimeTravel {
    should_record: bool,
    moments: Vec<(StateAtTime, Text)>,
}

struct StateAtTime {
    time: Duration,
    cars: BTreeMap<CarID, DrawCarInput>,
    peds: BTreeMap<PedestrianID, DrawPedestrianInput>,
    cars_per_traversable: MultiMap<Traversable, CarID>,
    peds_per_traversable: MultiMap<Traversable, PedestrianID>,
}

impl InactiveTimeTravel {
    pub fn new() -> InactiveTimeTravel {
        InactiveTimeTravel {
            should_record: false,
            moments: Vec::new(),
        }
    }

    pub fn start(&mut self, ctx: &mut EventCtx, ui: &UI) -> Transition {
        self.should_record = true;
        // In case we weren't already...
        self.record(ui);

        // Temporarily move our state into the new one.
        let items = std::mem::replace(&mut self.moments, Vec::new());

        Transition::Push(Box::new(TimeTraveler {
            slider: ItemSlider::new(
                items,
                "Time Traveler",
                "moment",
                vec![vec![(hotkey(Key::Escape), "quit")]],
                ctx,
            ),
        }))
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

        if let Some((ref state, _)) = self.moments.last() {
            // Already have this
            if now == state.time {
                return;
            }
            // We just loaded a new savestate or reset or something. Clear out our memory.
            if now < state.time {
                self.moments.clear();
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
        let label = Text::from(Line(state.time.to_string()));
        self.moments.push((state, label));
    }
}

struct TimeTraveler {
    slider: ItemSlider<StateAtTime>,
}

impl State for TimeTraveler {
    // Returns true if done.
    fn event(&mut self, ctx: &mut EventCtx, _: &mut UI) -> Transition {
        self.slider.event(ctx);
        ctx.canvas.handle_event(ctx.input);

        if self.slider.action("quit") {
            let moments = self.slider.consume_all_items();
            return Transition::PopWithData(Box::new(|state, _, _| {
                let mut sandbox = state.downcast_mut::<SandboxMode>().unwrap();
                sandbox.time_travel.moments = moments;
            }));
        }
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        ui.draw(g, DrawOptions::new(), self, &ShowEverything::new());
        self.slider.draw(g);
    }
}

impl TimeTraveler {
    fn get_current_state(&self) -> &StateAtTime {
        self.slider.get().1
    }
}

impl GetDrawAgents for TimeTraveler {
    fn time(&self) -> Duration {
        self.get_current_state().time
    }

    fn step_count(&self) -> usize {
        self.slider.get().0
    }

    fn get_draw_car(&self, id: CarID, _: &Map) -> Option<DrawCarInput> {
        self.get_current_state().cars.get(&id).cloned()
    }

    fn get_draw_ped(&self, id: PedestrianID, _: &Map) -> Option<DrawPedestrianInput> {
        self.get_current_state().peds.get(&id).cloned()
    }

    fn get_draw_cars(&self, on: Traversable, _: &Map) -> Vec<DrawCarInput> {
        let state = self.get_current_state();
        // TODO sort by ID to be deterministic?
        state
            .cars_per_traversable
            .get(on)
            .iter()
            .map(|id| state.cars[id].clone())
            .collect()
    }

    // TODO This cheats and doesn't handle crowds. :\
    fn get_draw_peds(
        &self,
        on: Traversable,
        _: &Map,
    ) -> (Vec<DrawPedestrianInput>, Vec<DrawPedCrowdInput>) {
        let state = self.get_current_state();
        (
            state
                .peds_per_traversable
                .get(on)
                .iter()
                .map(|id| state.peds[id].clone())
                .collect(),
            Vec::new(),
        )
    }

    fn get_all_draw_cars(&self, _: &Map) -> Vec<DrawCarInput> {
        self.get_current_state().cars.values().cloned().collect()
    }

    fn get_all_draw_peds(&self, _: &Map) -> Vec<DrawPedestrianInput> {
        self.get_current_state().peds.values().cloned().collect()
    }

    fn get_unzoomed_agents(&self, _: &Map) -> Vec<UnzoomedAgent> {
        // TODO Doesn't work yet.
        Vec::new()
    }
}
