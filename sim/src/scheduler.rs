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
}

impl Scheduler {
    pub fn new() -> Scheduler {
        Scheduler { items: Vec::new() }
    }

    pub fn push(&mut self, time: Duration, cmd: Command) {
        // TODO Implement more efficiently
        self.items.push((time, cmd));
        self.items.sort_by_key(|(t, _)| *t);
        self.items.reverse();
    }

    pub fn update(&mut self, cmd: Command, new_time: Duration) {
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
        Some(self.items.pop().unwrap().1)
    }
}
