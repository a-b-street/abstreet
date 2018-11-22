use map_model::Map;
use objects::SIM;
use piston::input::Key;
use plugins::{Plugin, PluginCtx};
use sim::{AgentID, CarID, DrawCarInput, DrawPedestrianInput, PedestrianID, Sim, Tick};
use std::collections::BTreeMap;

pub struct TimeTravel {
    state_per_tick: Vec<StateAtTime>,
    current_tick: Option<Tick>,
}

struct StateAtTime {
    cars: BTreeMap<CarID, DrawCarInput>,
    peds: BTreeMap<PedestrianID, DrawPedestrianInput>,
}

impl TimeTravel {
    pub fn new() -> TimeTravel {
        TimeTravel {
            state_per_tick: Vec::new(),
            current_tick: None,
        }
    }

    fn record_state(&mut self, sim: &Sim, map: &Map) {
        // Record state for this tick, if needed.
        let tick = sim.time.as_usize();
        if tick + 1 == self.state_per_tick.len() {
            return;
        }
        assert_eq!(tick, self.state_per_tick.len());

        let mut state = StateAtTime {
            cars: BTreeMap::new(),
            peds: BTreeMap::new(),
        };
        for agent in sim.active_agents().into_iter() {
            match agent {
                AgentID::Car(id) => {
                    state.cars.insert(id, sim.get_draw_car(id, map).unwrap());
                }
                AgentID::Pedestrian(id) => {
                    state.peds.insert(id, sim.get_draw_ped(id, map).unwrap());
                }
            };
        }
        self.state_per_tick.push(state);
    }
}

impl Plugin for TimeTravel {
    fn event(&mut self, ctx: PluginCtx) -> bool {
        self.record_state(&ctx.primary.sim, &ctx.primary.map);

        if let Some(tick) = self.current_tick {
            if tick != Tick::zero() && ctx.input.key_pressed(Key::Comma, "rewind") {
                self.current_tick = Some(tick.prev());
            } else if tick.as_usize() + 1 < self.state_per_tick.len()
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

        self.current_tick.is_some()
    }

    // TODO show current tick in OSD
}
