use crate::{Command, Event, Person, PersonID, Scheduler};
use geom::{Duration, Time};
use map_model::BuildingID;
use rand::Rng;
use rand_xorshift::XorShiftRng;
use serde_derive::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone)]
pub struct PandemicModel {
    pub infected: BTreeSet<PersonID>,
    hospitalized: BTreeSet<PersonID>,
    // Since when has a person been inside a building?
    // TODO This is an awkward data structure; abstutil::MultiMap is also bad, because key removal
    // would require knowing the time. Want something closer to
    // https://guava.dev/releases/19.0/api/docs/com/google/common/collect/Table.html.
    bldg_occupants: BTreeMap<BuildingID, Vec<(PersonID, Time)>>,

    rng: XorShiftRng,
    initialized: bool,
}

// You can schedule callbacks in the future by doing scheduler.push(future time, one of these)
#[derive(Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Clone, Debug)]
pub enum Cmd {
    BecomeHospitalized(PersonID),
}

// TODO Pretend handle_event and handle_cmd also take in some object that lets you do things like:
//
// - replace_future_trips(PersonID, Vec<IndividTrip>)
//
// I'm not exactly sure how this should work yet. Any place you want to change the rest of the
// simulation, just add a comment describing what you want to do exactly, and we'll figure it out
// from there.

impl PandemicModel {
    pub fn new(rng: XorShiftRng) -> PandemicModel {
        PandemicModel {
            infected: BTreeSet::new(),
            hospitalized: BTreeSet::new(),
            bldg_occupants: BTreeMap::new(),
            rng,
            initialized: false,
        }
    }

    // Sorry, initialization order of simulations is still a bit messy. This'll be called at
    // Time::START_OF_DAY after all of the people have been created from a Scenario.
    pub fn initialize(&mut self, population: &Vec<Person>, scheduler: &mut Scheduler) {
        assert!(!self.initialized);
        self.initialized = true;

        // Seed initially infected people.
        for p in population {
            if self.rng.gen_bool(0.1) {
                self.become_infected(Time::START_OF_DAY, p.id, scheduler);
            }
        }
    }

    pub fn handle_event(&mut self, now: Time, ev: &Event, scheduler: &mut Scheduler) {
        assert!(self.initialized);
        // TODO Handle bus events

        match ev {
            Event::PersonEntersBuilding(p, b) => {
                let person = *p;
                let bldg = *b;

                self.bldg_occupants
                    .entry(bldg)
                    .or_insert_with(Vec::new)
                    .push((person, now));
            }
            Event::PersonLeavesBuilding(p, b) => {
                let person = *p;
                let bldg = *b;

                // TODO Messy to mutate state inside a retain closure
                let mut inside_since: Option<Time> = None;
                self.bldg_occupants
                    .entry(bldg)
                    .or_insert_with(Vec::new)
                    .retain(|(p, t)| {
                        if *p == person {
                            inside_since = Some(*t);
                            false
                        } else {
                            true
                        }
                    });
                // TODO A person left a building, but they weren't inside of it? Bug -- few
                // possible causes...
                if inside_since.is_none() {
                    return;
                }
                let inside_since = inside_since.unwrap();

                // Was this person leaving infected while they were inside?
                if !self.infected.contains(&person) {
                    let mut longest_overlap_with_infected = Duration::ZERO;
                    for (p, t) in &self.bldg_occupants[&bldg] {
                        if !self.infected.contains(p) {
                            continue;
                        }
                        // How much time was p inside the building with person?
                        let dt = now - (*t).max(inside_since);
                        longest_overlap_with_infected = longest_overlap_with_infected.max(dt);
                    }
                    if longest_overlap_with_infected > Duration::hours(1) && self.rng.gen_bool(0.1)
                    {
                        self.become_infected(now, person, scheduler);
                    }
                }
            }
            _ => {}
        }
    }

    pub fn handle_cmd(&mut self, _now: Time, cmd: Cmd, _scheduler: &mut Scheduler) {
        assert!(self.initialized);

        match cmd {
            Cmd::BecomeHospitalized(person) => {
                self.hospitalized.insert(person);
            }
        }
    }

    fn become_infected(&mut self, now: Time, person: PersonID, scheduler: &mut Scheduler) {
        self.infected.insert(person);

        if self.rng.gen_bool(0.1) {
            scheduler.push(
                now + self.rand_duration(Duration::hours(1), Duration::hours(3)),
                Command::Pandemic(Cmd::BecomeHospitalized(person)),
            );
        }
    }

    fn rand_duration(&mut self, low: Duration, high: Duration) -> Duration {
        assert!(high > low);
        Duration::seconds(
            self.rng
                .gen_range(low.inner_seconds(), high.inner_seconds()),
        )
    }
}
