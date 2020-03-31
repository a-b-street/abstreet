use crate::{Analytics, PersonID};
use geom::{Duration, Time};
use map_model::BuildingID;
use rand::Rng;
use rand_xorshift::XorShiftRng;
use std::collections::{BTreeMap, BTreeSet};

const T_INF: f64 = 3600.0 * 24.0 * 5.0;
const T_INC: f64 = 3600.0 * 24.0 * 5.0;
const R_0: f64 = 2.5;

pub struct PandemicModel {
    // TODO For the moment let's develop everything with the SEIR model and refactor
    pub sane: BTreeSet<PersonID>,
    pub exposed: BTreeMap<PersonID, Time>,
    pub infected: BTreeMap<PersonID, Time>,
    pub recovered: BTreeSet<PersonID>,
    // Since when has a person been inside a building?
    // TODO This is an awkward data structure; abstutil::MultiMap is also bad, because key removal
    // would require knowing the time. Want something closer to
    // https://guava.dev/releases/19.0/api/docs/com/google/common/collect/Table.html.
    bldg_occupants: BTreeMap<BuildingID, Vec<(PersonID, Time)>>,
    t_inf: f64,
    t_inc: f64, 
    r0: f64,
}

impl PandemicModel {
    fn new() -> Self {
        PandemicModel {
            sane: BTreeSet::new(),
            exposed: BTreeMap::new(),
            infected: BTreeMap::new(),
            recovered: BTreeSet::new(),
            bldg_occupants: BTreeMap::new(),
            t_inf: T_INF,
            t_inc: T_INC,
            r0: R_0,
        }
    }

    // I think this general pattern makes the most sense. Unless we want to treat the pandemic
    // model as a first-class part of the main traffic simulation, we don't really need to put the
    // state in the rest of the sim crate. When the UI wants to do some reporting, we just read
    // events and figure out the state of the pandemic model at some time.
    //
    // This recomputes everything every time the UI asks for it. That's fine for the scale of
    // simulations now; everything else in Analytics works the same way. The faster streaming
    // version is very straightforward -- cache this output and only process new events.
    pub fn calculate(analytics: &Analytics, now: Time, rng: &mut XorShiftRng) -> PandemicModel {
        let mut state = PandemicModel::new();

        // Track people's movements through buildings
        for (time, person, bldg, left) in &analytics.building_transitions {
            if *time > now {
                break;
            }


            if *left { // person left building let's (let's see its contacts)
                // TODO Messy to mutate state inside a retain closure
                let mut inside_since: Option<Time> = None;
                state
                    .bldg_occupants
                    .entry(*bldg)
                    .or_insert_with(Vec::new)
                    .retain(|(p, t)| {
                        if *p == *person {
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
                if state.sane.contains(person) {
                    //let time_in_bldg = time - inside_since.unwrap();
                    let mut longest_overlap_with_infected = Duration::ZERO;
                    for (p, t) in &state.bldg_occupants[bldg] {
                        if !state.infected.contains_key(p) {
                            continue;
                        }
                        // How much time was p inside the building with person?
                        let dt = *time - (*t).max(inside_since);
                        longest_overlap_with_infected = longest_overlap_with_infected.max(dt);
                    }
                    if rng.gen_bool(1.0 - (- longest_overlap_with_infected.inner_seconds() * state.r0 / state.t_inf).exp()) {
                        state.exposed.insert(*person, *time);
                    }
                }

                // Not perfect because we are only considering people leaving building
                // this should be performed by listening to any event actually (let's see how to get that)
                if let Some(t0) = state.infected.get(person) {
                    let dt = now - *t0;
                    let m = dt.inner_seconds() - state.t_inf;
                    let n = state.t_inf / 4.0;

                    let prob = libm::erf(0.5 * m * f64::sqrt(2.0)/n.sqrt()) - libm::erf(0.5 * f64::sqrt(2.0) * (-dt.inner_seconds() + m) / n.sqrt());

                    if rng.gen_bool(prob) {
                        state.recovered.insert(*person);
                    }
                }

                // Not perfect because we are only considering people leaving building
                // this should be performed by listening to any event actually (let's see how to get that)
                if let Some(t0) = state.exposed.get(person) {
                    let dt = now - *t0;
                    let m = dt.inner_seconds() - state.t_inc;
                    let n = state.t_inf / 4.0;

                    let prob = libm::erf(0.5 * m * f64::sqrt(2.0)/n.sqrt()) - libm::erf(0.5 * f64::sqrt(2.0) * (-dt.inner_seconds() + m) / n.sqrt());

                    if rng.gen_bool(prob) {
                        state.recovered.insert(*person);
                    }
                }
            } else {
                state
                    .bldg_occupants
                    .entry(*bldg)
                    .or_insert_with(Vec::new)
                    .push((*person, *time));

                // Bit of a hack to seed initial state per person here, but eh
                if *time == Time::START_OF_DAY {
                    if rng.gen_bool(0.1) {
                        state.exposed.insert(*person, *time);
                    }
                }
            }
        }

        state
    }
}
