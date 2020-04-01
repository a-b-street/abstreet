use crate::{Analytics, PersonID};
use geom::{Duration, Time};
use map_model::BuildingID;
use rand::Rng;
use rand_xorshift::XorShiftRng;
use std::collections::{BTreeMap, BTreeSet};

const T_INF: f64 = 3600.0 * 24.0 * 7.0; // TODO dummy values
const T_INC: f64 = 3600.0; // TODO dummy values
const R_0: f64 = 2.5;
const I_RATIO: f64 = 0.01;
const E_RATIO: f64 = I_RATIO / 2.0;

pub struct PandemicModel {
    // TODO For the moment let's develop everything with the SEIR model and refactor
    pub exposed: BTreeMap<PersonID, (Time, Time)>,
    pub infected: BTreeMap<PersonID, (Time, Time)>,
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

fn bad_proba(prob: f64) {
    if prob < 0.0 && prob > 1.0 {
        panic!("Bad probability {}", prob);
    }
}

fn bad_time_order(t0: f64, t1: f64) {
    if t0 > t1 {
        panic!("Bad time ordering {} {}", t0, t1);
    }
}

impl PandemicModel {
    fn new() -> Self {
        PandemicModel {
            exposed: BTreeMap::new(),
            infected: BTreeMap::new(),
            recovered: BTreeSet::new(),
            bldg_occupants: BTreeMap::new(),
            t_inf: T_INF,
            t_inc: T_INC,
            r0: R_0,
        }
    }

    fn proba_s_to_e(&self, time: f64) -> f64 {
        let prob = 1.0 - (-time * self.r0 / self.t_inf).exp();
        bad_proba(prob);
        prob
    }

    // goind from -infinity to t
    fn erf_distrib(t: f64, mu: f64, sigma: f64) -> f64 {
        0.5 - 0.5 * libm::erf((-t + mu) / (f64::sqrt(2.0) * sigma))
    }

    fn erf_distrib_bounded(t0: f64, t1: f64, mu: f64, sigma: f64) -> f64 {
        bad_time_order(t0, t1);
        0.5*libm::erf((-t0+mu)/(f64::sqrt(2.0) * sigma)) - 0.5*libm::erf((-t1+mu)/(f64::sqrt(2.0) * sigma))
    }
    

    fn proba_e_to_i(&self, time: f64) -> f64 {
        let prob = Self::erf_distrib(time, self.t_inc, self.t_inc / 4.0);
        bad_proba(prob);
        prob
    }

    fn proba_i_to_r(&self, time: f64) -> f64 {
        let prob = Self::erf_distrib(time, self.t_inf, self.t_inf / 4.0);
        bad_proba(prob);
        prob
    }
    

    fn proba_e_to_i_bounded(&self, ti: f64, t0: f64, t1: f64) -> f64 {
        let prob = Self::erf_distrib_bounded(t0, t1, ti + self.t_inc, self.t_inc / 4.0);
        bad_proba(prob);
        prob
    }

    fn proba_i_to_r_bounded(&self, ti: f64, t0: f64, t1: f64) -> f64 {
        let prob = Self::erf_distrib_bounded(t0, t1, ti + self.t_inf, self.t_inf / 4.0);
        bad_proba(prob);
        prob
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

            if *left {
                // person left building let's (let's see its contacts)
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
                if !state.infected.contains_key(person)
                    && !state.infected.contains_key(person)
                    && !state.recovered.contains(person)
                {
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
                    if rng
                        .gen_bool(state.proba_s_to_e(longest_overlap_with_infected.inner_seconds()))
                    {
                        // implifitly the person was sane before
                        state.exposed.insert(*person, (*time, *time));
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
                    if rng.gen_bool(E_RATIO) {
                        // TODO ideally we would like to have negative times for intialisation

                        // let rnd_time: f64 = rng.gen::<f64>() * state.t_inf;
                        // state.exposed.insert(*person, Time::START_OF_DAY - Duration::seconds(rnd_time));
                        state.exposed.insert(*person, (Time::START_OF_DAY, Time::START_OF_DAY));
                    } else if rng.gen_bool(I_RATIO) {
                        // TODO ideally we would like to have negative times for intialisation
                        // let rnd_time: f64 = rng.gen::<f64>() * state.t_inc;
                        // state.infected.insert(*person, Time::START_OF_DAY - Duration::seconds(rnd_time));
                        state.infected.insert(*person, (Time::START_OF_DAY, Time::START_OF_DAY));
                    }
                }
            }

            // Not perfect because we are only considering people entering/leaving buildings
            // this should be performed by listening to any event actually (let's see how to get that)
            // Transition I -> R
            let inf_pers = {
                state.infected.get(person).clone()
            };
            if let Some((t0, last_check)) = inf_pers {
                // let dt = now - *t0;
                if rng.gen_bool(state.proba_i_to_r_bounded(t0.inner_seconds(), last_check.inner_seconds(), time.inner_seconds())) {
                    state.recovered.insert(*person);
                    state.infected.remove(person);
                } else {
                    // We rather store the last moment
                    state.infected.insert(*person, (*t0, *time));
                }
            }

            // Not perfect because we are only considering people leaving building
            // this should be performed by listening to any event actually (let's see how to get that)
            // Transition E -> I
            let exp_pers = {
                state.exposed.get(person).clone()
            };
            if let Some((t0, last_check)) = exp_pers {
                // let dt = now - *t0;
                let prob = state.proba_e_to_i_bounded(t0.inner_seconds(), last_check.inner_seconds(), time.inner_seconds());
                println!("prob = {}", prob);
                if rng.gen_bool(prob) {
                    state.infected.insert(*person, (*time, *time));
                    state.exposed.remove(person);
                } else {
                    // We rather store the last moment
                    state.exposed.insert(*person, (*t0, *time));
                }
            }
        }

        state
    }
}
