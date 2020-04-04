use crate::pmodel::{erf_distrib_bounded, proba_decaying_sigmoid, SEIR};
use crate::{CarID, Command, Event, Person, PersonID, Scheduler, TripPhaseType};
use geom::{Duration, Time};
use map_model::{BuildingID, BusStopID};
use rand::Rng;
use rand_xorshift::XorShiftRng;
use serde_derive::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

// TODO This does not model transmission by surfaces; only person-to-person.
// TODO If two people are in the same shared space indefinitely and neither leaves, we don't model
// transmission. It only occurs when people leave a space.

#[derive(Clone)]
pub struct PandemicModel {
    pub sane: BTreeSet<PersonID>,
    // first time is the time of exposition/infection
    // second time is the time sine the last chek of
    // transition was performed
    pub exposed: BTreeMap<PersonID, (Time, Time)>,
    pub infected: BTreeMap<PersonID, (Time, Time)>,
    pub recovered: BTreeSet<PersonID>,
    hospitalized: BTreeSet<PersonID>,
    quarantined: BTreeSet<PersonID>,

    bldgs: SharedSpace<BuildingID>,
    bus_stops: SharedSpace<BusStopID>,
    buses: SharedSpace<CarID>,
    person_to_bus: BTreeMap<PersonID, CarID>,

    rng: XorShiftRng,
    initialized: bool,
}

// You can schedule callbacks in the future by doing scheduler.push(future time, one of these)
#[derive(Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Clone, Debug)]
pub enum Cmd {
    BecomeHospitalized(PersonID),
    BecomeQuarantined(PersonID),
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
            sane: BTreeSet::new(),
            exposed: BTreeMap::new(),
            infected: BTreeMap::new(),
            hospitalized: BTreeSet::new(),
            quarantined: BTreeSet::new(),
            recovered: BTreeSet::new(),

            bldgs: SharedSpace::new(),
            bus_stops: SharedSpace::new(),
            buses: SharedSpace::new(),
            person_to_bus: BTreeMap::new(),

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
            self.sane.insert(p.id);
            if self.rng.gen_bool(SEIR::get_initial_ratio(SEIR::Exposed)) {
                self.become_exposed(Time::START_OF_DAY, p.id, scheduler);
            } else if self.rng.gen_bool(SEIR::get_initial_ratio(SEIR::Infectious)) {
                self.sane.remove(&p.id);
                self.become_infected(Time::START_OF_DAY, p.id, scheduler);
            } else if self.rng.gen_bool(SEIR::get_initial_ratio(SEIR::Recovered)) {
                self.sane.insert(p.id);
                self.become_recovered(Time::START_OF_DAY, p.id, scheduler);
            }
        }
    }

    pub fn count_sane(&self) -> usize {
        self.sane.len()
    }

    pub fn count_exposed(&self) -> usize {
        self.exposed.len()
    }

    pub fn count_infected(&self) -> usize {
        self.infected.len()
    }

    pub fn count_recovered(&self) -> usize {
        self.recovered.len()
    }

    pub fn count_total(&self) -> usize {
        self.count_sane() + self.count_exposed() + self.count_infected() + self.count_recovered()
    }

    pub fn handle_event(&mut self, now: Time, ev: &Event, scheduler: &mut Scheduler) {
        assert!(self.initialized);

        match ev {
            Event::PersonEntersBuilding(person, bldg) => {
                self.bldgs.person_enters_space(now, *person, *bldg);
            }
            Event::PersonLeavesBuilding(person, bldg) => {
                if let Some(others) = self.bldgs.person_leaves_space(now, *person, *bldg) {
                    self.transmission(now, *person, others, scheduler);
                } else {
                    // TODO A person left a building, but they weren't inside of it? Not sure
                    // what's happening here yet.
                }
            }
            Event::TripPhaseStarting(_, p, _, _, tpt) => {
                let person = *p;
                match tpt {
                    TripPhaseType::WaitingForBus(_, stop) => {
                        self.bus_stops.person_enters_space(now, person, *stop);
                    }
                    TripPhaseType::RidingBus(_, stop, bus) => {
                        let others = self
                            .bus_stops
                            .person_leaves_space(now, person, *stop)
                            .unwrap();
                        self.transmission(now, person, others, scheduler);

                        self.buses.person_enters_space(now, person, *bus);
                        self.person_to_bus.insert(person, *bus);
                    }
                    TripPhaseType::Walking => {
                        // A person can start walking for many reasons, but the only possible state
                        // transition after riding a bus is walking, so use this to detect the end
                        // of a bus ride.
                        if let Some(car) = self.person_to_bus.remove(&person) {
                            let others = self.buses.person_leaves_space(now, person, car).unwrap();
                            self.transmission(now, person, others, scheduler);
                        }
                    }
                    _ => {
                        self.transition(now, person, scheduler);
                    }
                }
            }
            _ => {}
        }
    }

    pub fn handle_cmd(&mut self, _now: Time, cmd: Cmd, _scheduler: &mut Scheduler) {
        assert!(self.initialized);

        // TODO Here we might enforce policies. Like severe -> become hospitalized
        // Symptomatic -> stay quaratined, and/or track contacts to quarantine them too (or test them)
        match cmd {
            Cmd::BecomeHospitalized(person) => {
                self.hospitalized.insert(person);
            }
            Cmd::BecomeQuarantined(person) => {
                self.quarantined.insert(person);
            }
        }
    }

    fn transmission(
        &mut self,
        now: Time,
        person: PersonID,
        other_occupants: Vec<(PersonID, Duration)>,
        scheduler: &mut Scheduler,
    ) {
        // person has spent some duration in the same space as other people. Does transmission
        // occur?
        for (other, overlap) in other_occupants {
            if let Some(pid) = self.might_become_exposed(person, other) {
                if self.exposition_occurs(overlap) {
                    self.become_exposed(now, pid, scheduler);
                }
            }
        }
    }

    // transition from a state to another without interaction with others
    fn transition(&mut self, now: Time, person: PersonID, scheduler: &mut Scheduler) {
        // person has spent some duration in the same space as other people. Does transmission
        // occur?
        let inf_pers = self.infected.get(&person).map(|pers| *pers);
        if let Some((t0, last_check)) = inf_pers {
            // let dt = now - *t0;
            if self.recovery_occurs(
                last_check,
                now,
                t0 + SEIR::get_transition_time_from(SEIR::Infectious),
                SEIR::get_transition_time_uncertainty_from(SEIR::Infectious),
            ) {
                self.transition_to_recovered(now, person, scheduler);
            } else {
                // We rather store the last moment
                self.stay_infected(t0, now, person, scheduler);
            }
        }

        let exp_pers = self.exposed.get(&person).map(|pers| *pers);
        if let Some((t0, last_check)) = exp_pers {
            // let dt = now - *t0;
            if self.infection_occurs(
                last_check,
                now,
                t0 + SEIR::get_transition_time_from(SEIR::Infectious),
                SEIR::get_transition_time_uncertainty_from(SEIR::Infectious),
            ) {
                self.transition_to_infected(now, person, scheduler);
            } else {
                // We rather store the last moment
                self.stay_exposed(t0, now, person, scheduler);
            }
        }
    }

    fn might_become_exposed(&self, person: PersonID, other: PersonID) -> Option<PersonID> {
        if self.infected.contains_key(&person) && self.sane.contains(&other) {
            return Some(other);
        } else if self.sane.contains(&person) && self.infected.contains_key(&other) {
            return Some(person);
        }
        None
    }

    // recovery occurs on average after some time (the probability is given between t0, t1),
    // but we must take into accoutn the avg moment of recovery and some uncertainty
    fn recovery_occurs(&mut self, t0: Time, t1: Time, avg_time: Time, sig_time: Duration) -> bool {
        self.rng.gen_bool(erf_distrib_bounded(
            t0.inner_seconds(),
            t1.inner_seconds(),
            avg_time.inner_seconds(),
            sig_time.inner_seconds(),
        ))
    }

    // infection occurs on average after some time (the probability is given between t0, t1),
    // but we must take into accoutn the avg moment of infection and some uncertainty
    fn infection_occurs(&mut self, t0: Time, t1: Time, avg_time: Time, sig_time: Duration) -> bool {
        self.rng.gen_bool(erf_distrib_bounded(
            t0.inner_seconds(),
            t1.inner_seconds(),
            avg_time.inner_seconds(),
            sig_time.inner_seconds(),
        ))
    }

    // Infection is the transition
    fn exposition_occurs(&mut self, overlap: Duration) -> bool {
        let rate = 1.0 / SEIR::get_transition_time_from(SEIR::Sane).inner_seconds();
        self.rng
            .gen_bool(proba_decaying_sigmoid(overlap.inner_seconds(), rate))
    }

    fn become_exposed(&mut self, now: Time, person: PersonID, _scheduler: &mut Scheduler) {
        // TODO We might want to track that contact at some point
        // SO let's keep the scheduler here
        self.exposed.insert(person, (now, now));
        self.sane.remove(&person);
    }

    fn become_infected(&mut self, now: Time, person: PersonID, _scheduler: &mut Scheduler) {
        self.infected.insert(person, (now, now));
    }

    fn become_recovered(&mut self, _now: Time, person: PersonID, _scheduler: &mut Scheduler) {
        self.recovered.insert(person);
    }

    fn transition_to_recovered(&mut self, _now: Time, person: PersonID, _scheduler: &mut Scheduler) {
        self.recovered.insert(person);
        self.infected.remove(&person);

        // if self.rng.gen_bool(0.1) {
        //     scheduler.push(
        //         now + self.rand_duration(Duration::hours(1), Duration::hours(3)),
        //         Command::Pandemic(Cmd::BecomeHospitalized(person)),
        //     );
        // }
    }

    fn transition_to_infected(&mut self, now: Time, person: PersonID, _scheduler: &mut Scheduler) {
        self.infected.insert(person, (now, now));
        self.exposed.remove(&person);

        // if self.rng.gen_bool(0.1) {
        //     scheduler.push(
        //         now + self.rand_duration(Duration::hours(1), Duration::hours(3)),
        //         Command::Pandemic(Cmd::BecomeHospitalized(person)),
        //     );
        // }
    }

    fn stay_infected(&mut self, ini: Time, now: Time, person: PersonID, _scheduler: &mut Scheduler) {
        self.infected.insert(person, (ini, now));
    }

    fn stay_exposed(&mut self, ini: Time, now: Time, person: PersonID, _scheduler: &mut Scheduler) {
        self.exposed.insert(person, (ini, now));
    }

    // fn rand_duration(&mut self, low: Duration, high: Duration) -> Duration {
    //     assert!(high > low);
    //     Duration::seconds(
    //         self.rng
    //             .gen_range(low.inner_seconds(), high.inner_seconds()),
    //     )
    // }
}

#[derive(Clone)]
struct SharedSpace<T: Ord> {
    // Since when has a person been in some shared space?
    // TODO This is an awkward data structure; abstutil::MultiMap is also bad, because key removal
    // would require knowing the time. Want something closer to
    // https://guava.dev/releases/19.0/api/docs/com/google/common/collect/Table.html.
    occupants: BTreeMap<T, Vec<(PersonID, Time)>>,
}

impl<T: Ord> SharedSpace<T> {
    fn new() -> SharedSpace<T> {
        SharedSpace {
            occupants: BTreeMap::new(),
        }
    }

    fn person_enters_space(&mut self, now: Time, person: PersonID, space: T) {
        self.occupants
            .entry(space)
            .or_insert_with(Vec::new)
            .push((person, now));
    }

    // Returns a list of all other people that the person was in the shared space with, and how
    // long their time overlapped. If it returns None, then a bug must have occurred, because
    // somebody has left a space they never entered.
    fn person_leaves_space(
        &mut self,
        now: Time,
        person: PersonID,
        space: T,
    ) -> Option<Vec<(PersonID, Duration)>> {
        // TODO Messy to mutate state inside a retain closure
        let mut inside_since: Option<Time> = None;
        let occupants = self.occupants.entry(space).or_insert_with(Vec::new);
        occupants.retain(|(p, t)| {
            if *p == person {
                inside_since = Some(*t);
                false
            } else {
                true
            }
        });
        // TODO Bug!
        let inside_since = inside_since?;

        Some(
            occupants
                .iter()
                .map(|(p, t)| (*p, now - (*t).max(inside_since)))
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn time(x: usize) -> Time {
        Time::START_OF_DAY + Duration::hours(x)
    }

    #[test]
    fn test_overlap() {
        let mut space = SharedSpace::new();
        let mut now = time(0);

        let bldg1 = BuildingID(1);
        let bldg2 = BuildingID(2);

        let person1 = PersonID(1);
        let person2 = PersonID(2);
        let person3 = PersonID(3);

        // Only one person
        space.person_enters_space(now, person1, bldg1);
        now = time(1);
        assert_eq!(
            space.person_leaves_space(now, person1, bldg1),
            Some(Vec::new())
        );

        // Two people at the same time
        now = time(2);
        space.person_enters_space(now, person1, bldg2);
        space.person_enters_space(now, person2, bldg2);
        now = time(3);
        assert_eq!(
            space.person_leaves_space(now, person1, bldg2),
            Some(vec![(person2, Duration::hours(1))])
        );

        // Bug
        assert_eq!(space.person_leaves_space(now, person3, bldg2), None);

        // Different times
        now = time(5);
        space.person_enters_space(now, person1, bldg1);
        now = time(6);
        space.person_enters_space(now, person2, bldg1);
        now = time(7);
        space.person_enters_space(now, person3, bldg1);
        now = time(10);
        assert_eq!(
            space.person_leaves_space(now, person1, bldg1),
            Some(vec![
                (person2, Duration::hours(4)),
                (person3, Duration::hours(3))
            ])
        );
        now = time(12);
        assert_eq!(
            space.person_leaves_space(now, person2, bldg1),
            Some(vec![(person3, Duration::hours(5))])
        );
    }
}
