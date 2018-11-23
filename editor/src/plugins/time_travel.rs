use abstutil::MultiMap;
use map_model::{LaneID, Map, Traversable, TurnID};
use objects::SIM;
use piston::input::Key;
use plugins::{Plugin, PluginCtx};
use sim::{CarID, DrawCarInput, DrawPedestrianInput, GetDrawAgents, PedestrianID, Sim, Tick};
use std::collections::BTreeMap;

pub struct TimeTravel {
    state_per_tick: Vec<StateAtTime>,
    current_tick: Option<Tick>,
    first_tick: Tick,
}

struct StateAtTime {
    cars: BTreeMap<CarID, DrawCarInput>,
    peds: BTreeMap<PedestrianID, DrawPedestrianInput>,
    cars_per_traversable: MultiMap<Traversable, CarID>,
    peds_per_traversable: MultiMap<Traversable, PedestrianID>,
}

impl TimeTravel {
    pub fn new(first_tick: Tick) -> TimeTravel {
        TimeTravel {
            state_per_tick: Vec::new(),
            current_tick: None,
            first_tick,
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
        assert_eq!(tick, self.first_tick.as_usize() + self.state_per_tick.len());

        let mut state = StateAtTime {
            cars: BTreeMap::new(),
            peds: BTreeMap::new(),
            cars_per_traversable: MultiMap::new(),
            peds_per_traversable: MultiMap::new(),
        };
        for l in map.all_lanes().iter() {
            if l.is_sidewalk() {
                for draw in sim.get_draw_peds_on_lane(l.id, map).into_iter() {
                    state
                        .peds_per_traversable
                        .insert(Traversable::Lane(l.id), draw.id);
                    state.peds.insert(draw.id, draw);
                }
            } else {
                for draw in sim.get_draw_cars_on_lane(l.id, map).into_iter() {
                    state
                        .cars_per_traversable
                        .insert(Traversable::Lane(l.id), draw.id);
                    state.cars.insert(draw.id, draw);
                }
            }
        }
        for t in map.all_turns().values() {
            if t.between_sidewalks() {
                for draw in sim.get_draw_peds_on_turn(t.id, map).into_iter() {
                    state
                        .peds_per_traversable
                        .insert(Traversable::Turn(t.id), draw.id);
                    state.peds.insert(draw.id, draw);
                }
            } else {
                for draw in sim.get_draw_cars_on_turn(t.id, map).into_iter() {
                    state
                        .cars_per_traversable
                        .insert(Traversable::Turn(t.id), draw.id);
                    state.cars.insert(draw.id, draw);
                }
            }
        }
        self.state_per_tick.push(state);
    }

    fn get_current_state(&self) -> &StateAtTime {
        &self.state_per_tick[self.current_tick.unwrap().as_usize() - self.first_tick.as_usize()]
    }
}

impl Plugin for TimeTravel {
    fn event(&mut self, ctx: PluginCtx) -> bool {
        self.record_state(&ctx.primary.sim, &ctx.primary.map);

        if let Some(tick) = self.current_tick {
            if tick > self.first_tick && ctx.input.key_pressed(Key::Comma, "rewind") {
                self.current_tick = Some(tick.prev());
            } else if tick.as_usize() + 1 < self.first_tick.as_usize() + self.state_per_tick.len()
                && ctx.input.key_pressed(Key::Period, "forward")
            {
                self.current_tick = Some(tick.next());
            } else if ctx.input.key_pressed(Key::Return, "exit time traveler") {
                self.current_tick = None;
            }
        } else {
            if ctx
                .input
                .unimportant_key_pressed(Key::T, SIM, "start time traveling")
            {
                self.current_tick = Some(ctx.primary.sim.time);
            }
        }

        if let Some(tick) = self.current_tick {
            ctx.osd.add_line(format!("Time traveling: {}", tick));
        }

        self.is_active()
    }
}

impl GetDrawAgents for TimeTravel {
    fn get_draw_car(&self, id: CarID, _map: &Map) -> Option<DrawCarInput> {
        self.get_current_state().cars.get(&id).map(|d| d.clone())
    }

    fn get_draw_ped(&self, id: PedestrianID, _map: &Map) -> Option<DrawPedestrianInput> {
        self.get_current_state().peds.get(&id).map(|d| d.clone())
    }

    fn get_draw_cars_on_lane(&self, l: LaneID, _map: &Map) -> Vec<DrawCarInput> {
        let state = self.get_current_state();
        // TODO sort by ID to be deterministic?
        state
            .cars_per_traversable
            .get(Traversable::Lane(l))
            .into_iter()
            .map(|id| state.cars[id].clone())
            .collect()
    }

    fn get_draw_cars_on_turn(&self, t: TurnID, _map: &Map) -> Vec<DrawCarInput> {
        let state = self.get_current_state();
        state
            .cars_per_traversable
            .get(Traversable::Turn(t))
            .into_iter()
            .map(|id| state.cars[id].clone())
            .collect()
    }

    fn get_draw_peds_on_lane(&self, l: LaneID, _map: &Map) -> Vec<DrawPedestrianInput> {
        let state = self.get_current_state();
        state
            .peds_per_traversable
            .get(Traversable::Lane(l))
            .into_iter()
            .map(|id| state.peds[id].clone())
            .collect()
    }

    fn get_draw_peds_on_turn(&self, t: TurnID, _map: &Map) -> Vec<DrawPedestrianInput> {
        let state = self.get_current_state();
        state
            .peds_per_traversable
            .get(Traversable::Turn(t))
            .into_iter()
            .map(|id| state.peds[id].clone())
            .collect()
    }
}
