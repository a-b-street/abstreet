use crate::{CarID, CreateCar, CreatePedestrian, PedestrianID};
use derivative::Derivative;
use geom::Duration;
use histogram::Histogram;
use map_model::IntersectionID;
use serde_derive::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, PartialEq)]
pub enum Command {
    SpawnCar(CreateCar),
    SpawnPed(CreatePedestrian),
    UpdateCar(CarID),
    UpdatePed(PedestrianID),
    UpdateIntersection(IntersectionID),
}

#[derive(Serialize, Deserialize, Derivative)]
#[derivative(PartialEq)]
pub struct Scheduler {
    // TODO Implement more efficiently. Last element has earliest time.
    items: Vec<(Duration, Command)>,

    latest_time: Duration,
    // TODO Why doesn't the Histogram keep a total count? :(
    num_events: usize,
    #[derivative(PartialEq = "ignore")]
    #[serde(skip_serializing, skip_deserializing)]
    delta_times: Histogram,
}

impl Scheduler {
    pub fn new() -> Scheduler {
        Scheduler {
            items: Vec::new(),
            latest_time: Duration::ZERO,
            num_events: 0,
            delta_times: Histogram::new(),
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
        self.delta_times
            .increment((time - self.latest_time).to_u64())
            .unwrap();

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
    pub fn get_next(&mut self, now: Duration) -> Option<(Command, Duration)> {
        let next_time = self.items.last().as_ref()?.0;
        if next_time > now {
            return None;
        }
        self.latest_time = next_time;
        Some((self.items.pop().unwrap().1, next_time))
    }

    pub fn describe_stats(&self) -> String {
        format!(
            "{} events pushed, delta times: 50%ile {:?}, 90%ile {:?}, 99%ile {:?}",
            abstutil::prettyprint_usize(self.num_events),
            Duration::from_u64(self.delta_times.percentile(50.0).unwrap()),
            Duration::from_u64(self.delta_times.percentile(90.0).unwrap()),
            Duration::from_u64(self.delta_times.percentile(99.0).unwrap()),
        )
    }
}
