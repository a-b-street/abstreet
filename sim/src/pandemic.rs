use crate::{Analytics, Person, PersonID};
use geom::{Duration, Time};
use map_model::BuildingID;
use rand::Rng;
use rand_xorshift::XorShiftRng;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};

pub struct PandemicModel {
    pub infected: BTreeSet<PersonID>,
    hospitalized: BTreeSet<PersonID>,
    // Since when has a person been inside a building?
    // TODO This is an awkward data structure; abstutil::MultiMap is also bad, because key removal
    // would require knowing the time. Want something closer to
    // https://guava.dev/releases/19.0/api/docs/com/google/common/collect/Table.html.
    bldg_occupants: BTreeMap<BuildingID, Vec<(PersonID, Time)>>,

    events: BinaryHeap<Item>,
}

impl PandemicModel {
    // I think this general pattern makes the most sense. Unless we want to treat the pandemic
    // model as a first-class part of the main traffic simulation, we don't really need to put the
    // state in the rest of the sim crate. When the UI wants to do some reporting, we just read
    // events and figure out the state of the pandemic model at some time.
    //
    // This recomputes everything every time the UI asks for it. That's fine for the scale of
    // simulations now; everything else in Analytics works the same way. The faster streaming
    // version is very straightforward -- cache this output and only process new events.
    pub fn calculate(
        analytics: &Analytics,
        population: &Vec<Person>,
        now: Time,
        rng: &mut XorShiftRng,
    ) -> PandemicModel {
        let mut state = PandemicModel {
            infected: BTreeSet::new(),
            hospitalized: BTreeSet::new(),
            bldg_occupants: BTreeMap::new(),
            events: BinaryHeap::new(),
        };

        // Seed initially infected people.
        for p in population {
            if rng.gen_bool(0.1) {
                state.infected.insert(p.id);

                if rng.gen_bool(0.1) {
                    state.events.push(Item {
                        time: Time::START_OF_DAY
                            + rand_duration(rng, Duration::hours(1), Duration::hours(3)),
                        event: Event::Hospitalized(p.id),
                    });
                }
            }
        }

        // Seed events in the pandemic model from the traffic simulaton.
        for (time, person, bldg, left) in &analytics.building_transitions {
            if *time > now {
                break;
            }
            state.events.push(Item {
                time: *time,
                event: if *left {
                    Event::LeaveBldg(*person, *bldg)
                } else {
                    Event::EnterBldg(*person, *bldg)
                },
            });
        }

        // TODO Seed Event::EnterBus(person, car ID) and Seed::LeaveBus(person, car ID). Have to
        // remember some more info in Analytics first. The TripPhaseStarting event almost has
        // everything needed to build this. But the point is, the building and bus events can be
        // interleaved properly using the queue.

        // Process events in time-order
        while let Some(item) = state.events.pop() {
            let time = item.time;
            if time > now {
                break;
            }
            match item.event {
                Event::LeaveBldg(person, bldg) => {
                    // TODO Messy to mutate state inside a retain closure
                    let mut inside_since: Option<Time> = None;
                    state
                        .bldg_occupants
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
                        continue;
                    }
                    let inside_since = inside_since.unwrap();

                    // Was this person leaving infected while they were inside?
                    if !state.infected.contains(&person) {
                        let mut longest_overlap_with_infected = Duration::ZERO;
                        for (p, t) in &state.bldg_occupants[&bldg] {
                            if !state.infected.contains(p) {
                                continue;
                            }
                            // How much time was p inside the building with person?
                            let dt = time - (*t).max(inside_since);
                            longest_overlap_with_infected = longest_overlap_with_infected.max(dt);
                        }
                        if longest_overlap_with_infected > Duration::hours(1) && rng.gen_bool(0.1) {
                            state.infected.insert(person);
                            if rng.gen_bool(0.1) {
                                state.events.push(Item {
                                    time: time
                                        + rand_duration(
                                            rng,
                                            Duration::hours(1),
                                            Duration::hours(3),
                                        ),
                                    event: Event::Hospitalized(person),
                                });
                            }
                        }
                    }
                }
                Event::EnterBldg(person, bldg) => {
                    state
                        .bldg_occupants
                        .entry(bldg)
                        .or_insert_with(Vec::new)
                        .push((person, time));
                }
                Event::Hospitalized(person) => {
                    state.hospitalized.insert(person);
                    // TODO We need a way to tell the Sim to cancel any future trips. So there is a
                    // feedback loop... not sure the best way to structure this yet.
                }
            }
        }

        state
    }
}

#[derive(PartialEq, Eq)]
struct Item {
    time: Time,
    event: Event,
}

impl PartialOrd for Item {
    fn partial_cmp(&self, other: &Item) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Item {
    fn cmp(&self, other: &Item) -> Ordering {
        // BinaryHeap is a max-heap, so reverse the comparison to get smallest times first.
        let ord = other.time.cmp(&self.time);
        if ord != Ordering::Equal {
            return ord;
        }
        // The tie-breaker if time is the same is arbitrary, but deterministic
        self.event.cmp(&other.event)
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum Event {
    EnterBldg(PersonID, BuildingID),
    LeaveBldg(PersonID, BuildingID),
    Hospitalized(PersonID),
}

fn rand_duration(rng: &mut XorShiftRng, low: Duration, high: Duration) -> Duration {
    assert!(high > low);
    Duration::seconds(rng.gen_range(low.inner_seconds(), high.inner_seconds()))
}
