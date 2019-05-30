use crate::ui::UI;
use abstutil::MultiMap;
use ezgui::{hotkey, EventCtx, GfxCtx, ItemSlider, Key, Text};
use geom::Duration;
use map_model::{Map, Traversable};
use sim::{CarID, DrawCarInput, DrawPedestrianInput, GetDrawAgents, PedestrianID};
use std::collections::BTreeMap;

pub enum TimeTravel {
    Active(ItemSlider<StateAtTime>),
    Inactive {
        should_record: bool,
        moments: Vec<StateAtTime>,
    },
}

pub struct StateAtTime {
    time: Duration,
    cars: BTreeMap<CarID, DrawCarInput>,
    peds: BTreeMap<PedestrianID, DrawPedestrianInput>,
    cars_per_traversable: MultiMap<Traversable, CarID>,
    peds_per_traversable: MultiMap<Traversable, PedestrianID>,
}

impl TimeTravel {
    pub fn new() -> TimeTravel {
        TimeTravel::Inactive {
            should_record: false,
            moments: Vec::new(),
        }
    }

    pub fn start(&mut self, ctx: &mut EventCtx, ui: &UI) {
        // In case we weren't already...
        match self {
            TimeTravel::Inactive {
                ref mut should_record,
                ..
            } => {
                *should_record = true;
            }
            TimeTravel::Active(_) => unreachable!(),
        }
        self.record(ui);

        // TODO More cleanly?
        let items = match self {
            TimeTravel::Inactive {
                ref mut moments, ..
            } => std::mem::replace(moments, Vec::new()),
            TimeTravel::Active(_) => unreachable!(),
        };
        *self = TimeTravel::Active(ItemSlider::new(
            items,
            "Time Traveler",
            "moment",
            vec![(hotkey(Key::Escape), "quit")],
            ctx,
        ));
    }

    // TODO Now that we take big jumps forward in the source sim, the time traveler sees the same
    // granularity when replaying.
    pub fn record(&mut self, ui: &UI) {
        match self {
            TimeTravel::Inactive {
                ref should_record,
                ref mut moments,
            } => {
                if !*should_record {
                    return;
                }

                let map = &ui.primary.map;
                let sim = &ui.primary.sim;
                let now = sim.time();

                if let Some(ref state) = moments.last() {
                    // Already have this
                    if now == state.time {
                        return;
                    }
                    // We just loaded a new savestate or reset or something. Clear out our memory.
                    if now < state.time {
                        moments.clear();
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
                moments.push(state);
            }
            TimeTravel::Active(_) => unreachable!(),
        }
    }

    // Returns true if done.
    pub fn event(&mut self, ctx: &mut EventCtx) -> bool {
        match self {
            TimeTravel::Inactive { .. } => unreachable!(),
            TimeTravel::Active(ref mut slider) => {
                let (idx, state) = slider.get();
                let mut txt = Text::prompt("Time Traveler");
                txt.add_line(format!("{}", state.time));
                txt.add_line(format!("{}/{}", idx + 1, slider.len()));
                slider.event(ctx, Some(txt));
                ctx.canvas.handle_event(ctx.input);

                if slider.action("quit") {
                    *self = TimeTravel::Inactive {
                        should_record: true,
                        moments: slider.consume_all_items(),
                    };
                    return true;
                }
                false
            }
        }
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        match self {
            TimeTravel::Inactive { .. } => unreachable!(),
            TimeTravel::Active(ref slider) => {
                slider.draw(g);
            }
        }
    }

    fn get_current_state(&self) -> &StateAtTime {
        match self {
            TimeTravel::Inactive { .. } => unreachable!(),
            TimeTravel::Active(ref slider) => slider.get().1,
        }
    }
}

impl GetDrawAgents for TimeTravel {
    fn time(&self) -> Duration {
        self.get_current_state().time
    }

    fn step_count(&self) -> usize {
        match self {
            TimeTravel::Inactive { .. } => unreachable!(),
            TimeTravel::Active(ref slider) => slider.get().0,
        }
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
