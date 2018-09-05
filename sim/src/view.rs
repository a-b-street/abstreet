use driving::SimQueue;
use map_model::TurnID;
use std::collections::{BTreeMap, HashMap};
use {AgentID, CarID, Distance, On, Speed};

// An immutable view that agents and intersection controllers see of agents.
pub struct AgentView {
    pub id: AgentID,
    pub debug: bool,
    pub on: On,
    pub dist_along: Distance,
    pub speed: Speed,
}

pub struct WorldView {
    pub agents: HashMap<AgentID, AgentView>,

    // This is driving-specific state. Other ways of solving this:
    // - having a {Driving,Walking}WorldView and using the enum delegation trick (don't even really
    // need a macro; there's just three methods)
    // - make WalkingSimState also use SimQueues; they're overpowered for the current use, but
    // might be useful for understanding crowded sidewalks

    // TODO I want to borrow the SimQueues, not clone, but then react() still doesnt work to
    // mutably borrow router and immutably borrow the queues for the view. :(
    pub lanes: Vec<SimQueue>,
    pub turns: BTreeMap<TurnID, SimQueue>,
}

impl WorldView {
    pub fn new() -> WorldView {
        WorldView {
            agents: HashMap::new(),
            lanes: Vec::new(),
            turns: BTreeMap::new(),
        }
    }

    pub fn next_car_in_front_of(&self, on: On, dist: Distance) -> Option<&AgentView> {
        let maybe_id = match on {
            On::Lane(id) => self.lanes[id.0].next_car_in_front_of(dist),
            On::Turn(id) => self.turns[&id].next_car_in_front_of(dist),
        };
        maybe_id.map(|id| &self.agents[&AgentID::Car(id)])
    }

    pub fn is_leader(&self, id: AgentID) -> bool {
        match id {
            AgentID::Car(_) => {
                let c = &self.agents[&id];
                self.next_car_in_front_of(c.on, c.dist_along).is_none()
            }
            AgentID::Pedestrian(_) => true,
        }
    }

    pub fn get_speed(&self, id: AgentID) -> Speed {
        self.agents[&id].speed
    }

    pub fn get_car(&self, id: CarID) -> &AgentView {
        &self.agents[&AgentID::Car(id)]
    }
}
