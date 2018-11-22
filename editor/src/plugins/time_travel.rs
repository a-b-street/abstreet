use plugins::{Plugin, PluginCtx};
use map_model::Map;
use std::collections::BTreeMap;
use sim::{CarID, DrawCarInput, PedestrianID, DrawPedestrianInput, Sim, AgentID};

pub struct TimeTravel {
    state_per_tick: Vec<StateAtTime>,
}

struct StateAtTime {
    cars: BTreeMap<CarID, DrawCarInput>,
    peds: BTreeMap<PedestrianID, DrawPedestrianInput>,
}

impl TimeTravel {
    pub fn new() -> TimeTravel {
        TimeTravel {
            state_per_tick: Vec::new(),
        }
    }

    fn record_state(&mut self, sim: &Sim, map: &Map) {
        // Record state for this tick, if needed.
        let tick = sim.time.to_inner() as usize;
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

        false
    }
}
