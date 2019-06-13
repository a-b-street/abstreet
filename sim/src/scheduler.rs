use crate::{AgentID, CarID, CreateCar, CreatePedestrian, PedestrianID};
use derivative::Derivative;
use geom::{Duration, DurationHistogram};
use map_model::IntersectionID;
use serde_derive::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, PartialEq)]
pub enum Command {
    // If true, retry when there's no room to spawn somewhere
    SpawnCar(CreateCar, bool),
    SpawnPed(CreatePedestrian),
    UpdateCar(CarID),
    // Distinguish this from UpdateCar to avoid confusing things
    UpdateLaggyHead(CarID),
    UpdatePed(PedestrianID),
    UpdateIntersection(IntersectionID),
    CheckForGridlock,
    Savestate(Duration),
}

impl Command {
    pub fn update_agent(id: AgentID) -> Command {
        match id {
            AgentID::Car(c) => Command::UpdateCar(c),
            AgentID::Pedestrian(p) => Command::UpdatePed(p),
        }
    }
}

#[derive(Serialize, Deserialize, Derivative)]
#[derivative(PartialEq)]
pub struct Scheduler {
    // TODO Implement more efficiently. Last element has earliest time.
    items: Vec<(Duration, Command)>,

    latest_time: Duration,
    #[derivative(PartialEq = "ignore")]
    #[serde(skip_serializing, skip_deserializing)]
    delta_times: DurationHistogram,
}

impl Scheduler {
    pub fn new() -> Scheduler {
        Scheduler {
            items: Vec::new(),
            latest_time: Duration::ZERO,
            delta_times: std::default::Default::default(),
        }
    }

    pub fn push(&mut self, time: Duration, cmd: Command) {
        if time < self.latest_time {
            panic!(
                "It's at least {}, so can't schedule a command for {}",
                self.latest_time, time
            );
        }
        self.delta_times.add(time - self.latest_time);

        // TODO Make sure this is deterministic.
        // Note the order of comparison means times will be descending.
        let idx = match self.items.binary_search_by(|(at, _)| time.cmp(at)) {
            Ok(i) => i,
            Err(i) => i,
        };
        self.items.insert(idx, (time, cmd));
    }

    // Doesn't sort or touch the histogram. Have to call finalize_batch() after. Only for
    // scheduling lots of stuff at the beginning of a simulation.
    pub fn quick_push(&mut self, time: Duration, cmd: Command) {
        self.items.push((time, cmd));
    }

    pub fn finalize_batch(&mut self) {
        self.items.sort_by_key(|(time, _)| -*time);
    }

    pub fn update(&mut self, cmd: Command, new_time: Duration) {
        if new_time < self.latest_time {
            panic!(
                "It's at least {}, so can't schedule a command for {}",
                self.latest_time, new_time
            );
        }

        if let Some(idx) = self.items.iter().position(|(_, i)| *i == cmd) {
            self.items.remove(idx);
        }
        self.push(new_time, cmd);
    }

    pub fn cancel(&mut self, cmd: Command) {
        if let Some(idx) = self.items.iter().position(|(_, i)| *i == cmd) {
            self.items.remove(idx);
        }
    }

    // This API is safer than handing out a batch of items at a time, because while processing one
    // item, we might change the priority of other items or add new items. Don't make the caller
    // reconcile those changes -- just keep pulling items from here, one at a time.
    pub fn get_next(&mut self, now: Duration) -> Option<(Command, Duration)> {
        let next_time = self.items.last().as_ref()?.0;
        if next_time > now {
            return None;
        }
        self.latest_time = next_time;
        Some((self.items.pop().unwrap().1, next_time))
    }

    pub fn describe_stats(&self) -> String {
        format!("delta times for events: {}", self.delta_times.describe())
    }
}
