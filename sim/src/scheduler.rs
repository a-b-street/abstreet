use crate::{CarID, CreateCar, CreatePedestrian, PedestrianID};
use geom::Duration;
use serde_derive::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, PartialEq)]
pub enum Command {
    SpawnCar(CreateCar),
    SpawnPed(CreatePedestrian),
    UpdateCar(CarID),
    UpdatePed(PedestrianID),
}

#[derive(Serialize, Deserialize, PartialEq)]
pub struct Scheduler {
    // TODO Implement more efficiently. Last element has earliest time.
    items: Vec<(Duration, Command)>,

    latest_time: Duration,
    num_events: usize,
    // TODO More generally, track the distribution of delta-times from latest_time. Or even cooler,
    // per agent.
    num_epsilon_events: usize,
}

impl Scheduler {
    pub fn new() -> Scheduler {
        Scheduler {
            items: Vec::new(),
            latest_time: Duration::ZERO,
            num_events: 0,
            num_epsilon_events: 0,
        }
    }

    pub fn push(&mut self, time: Duration, cmd: Command) {
        if time < self.latest_time {
            panic!(
                "It's at least {}, so can't schedule a command for {}",
                self.latest_time, time
            );
        }
        self.num_events += 1;
        if time == self.latest_time + Duration::EPSILON {
            self.num_epsilon_events += 1;
        }

        // TODO Make sure this is deterministic.
        // Note the order of comparison means times will be descending.
        let idx = match self.items.binary_search_by(|(at, _)| time.cmp(at)) {
            Ok(i) => i,
            Err(i) => i,
        };
        self.items.insert(idx, (time, cmd));
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

    // This API is safer than handing out a batch of items at a time, because while processing one
    // item, we might change the priority of other items or add new items. Don't make the caller
    // reconcile those changes -- just keep pulling items from here, one at a time.
    pub fn get_next(&mut self, now: Duration) -> Option<Command> {
        let next_time = self.items.last().as_ref()?.0;
        // TODO Enable this validation after we're properly event-based. Right now, there are spawn
        // times between 0s and 0.1s, and stepping by 0.1s is too clunky.
        /*if next_time < now {
            panic!(
                "It's {}, but there's a command scheduled for {}",
                now, next_time
            );
        }*/
        if next_time > now {
            return None;
        }
        self.latest_time = next_time;
        Some(self.items.pop().unwrap().1)
    }

    pub fn describe_stats(&self) -> String {
        format!(
            "{} events pushed, {} of which only EPSILON in the future",
            abstutil::prettyprint_usize(self.num_events),
            abstutil::prettyprint_usize(self.num_epsilon_events)
        )
    }
}
