use driving::SimQueue;
use kinematics::Vehicle;
use map_model::Traversable;
use std::collections::{BTreeMap, HashMap};
use {AgentID, CarID, Distance, Speed};

// An immutable view that agents and intersection controllers see of agents.
pub struct AgentView {
    pub id: AgentID,
    pub debug: bool,
    pub on: Traversable,
    pub dist_along: Distance,
    pub speed: Speed,
    pub vehicle: Option<Vehicle>,
}

pub struct WorldView {
    pub agents: HashMap<AgentID, AgentView>,

    // This is driving-specific state. Other ways of solving this:
    // - having a {Driving,Walking}WorldView and using the enum delegation trick (don't even really
    // need a macro; there's just three methods)
    // - make WalkingSimState also use SimQueues; they're overpowered for the current use, but
    // might be useful for understanding crowded sidewalks
    pub queues: BTreeMap<Traversable, SimQueue>,
}

impl WorldView {
    pub fn new() -> WorldView {
        WorldView {
            agents: HashMap::new(),
            queues: BTreeMap::new(),
        }
    }

    pub fn next_car_in_front_of(&self, on: Traversable, dist: Distance) -> Option<&AgentView> {
        let queue = self.queues.get(&on)?;
        let id = queue.next_car_in_front_of(dist)?;
        Some(&self.agents[&AgentID::Car(id)])
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
