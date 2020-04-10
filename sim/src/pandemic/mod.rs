mod pandemic;
mod prob;

use geom::{Duration, Time};
pub use pandemic::{Cmd, PandemicModel};
pub use prob::{erf_distrib_bounded, proba_decaying_sigmoid};
use rand::Rng;
use rand_distr::{Distribution, Exp, Normal};
use rand_xorshift::XorShiftRng;
use std::ops;

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct AnyTime(f64);

impl AnyTime {
    fn inner_seconds(&self) -> f64 {
        self.0
    }

    pub fn is_finite(&self) -> bool {
        self.0.is_finite()
    }
}

impl ops::Add<Duration> for AnyTime {
    type Output = AnyTime;

    fn add(self, other: Duration) -> AnyTime {
        AnyTime(self.0 + other.inner_seconds())
    }
}

impl ops::AddAssign<Duration> for AnyTime {
    fn add_assign(&mut self, other: Duration) {
        *self = *self + other;
    }
}

impl ops::Sub<Duration> for AnyTime {
    type Output = AnyTime;

    fn sub(self, other: Duration) -> AnyTime {
        AnyTime(self.0 - other.inner_seconds())
    }
}

impl ops::Sub for AnyTime {
    type Output = Duration;

    fn sub(self, other: AnyTime) -> Duration {
        Duration::seconds(self.0 - other.0)
    }
}

impl From<Time> for AnyTime {
    fn from(t: Time) -> AnyTime {
        AnyTime(t.inner_seconds())
    }
}

impl From<f64> for AnyTime {
    fn from(t: f64) -> AnyTime {
        AnyTime(t)
    }
}

#[derive(Debug, Clone)]
pub enum State {
    Sane(Event),
    Exposed(Event),
    Infectious(Event),
    Hospitalized(Event),
    Recovered,
    Dead,
}

#[derive(Debug, Clone)]
pub enum StateEvent {
    Exposition,
    Incubation,
    Hospitalization,
    Recovery,
    Death,
}

#[derive(Debug, Clone)]
pub struct Event {
    s: StateEvent,
    p_hosp: f64,  // probability of people being hospitalized after infection
    p_death: f64, // probability of dying after hospitalizaion
    t: AnyTime,
}

impl Event {
    fn next(&self, now: AnyTime, rng: &mut XorShiftRng) -> State {
        match self.s {
            StateEvent::Exposition => State::Exposed(Event {
                s: StateEvent::Incubation,
                p_hosp: self.p_hosp,
                p_death: self.p_death,
                t: now + State::get_time_exp(rng),
            }),
            StateEvent::Incubation => {
                if rng.gen_bool(self.p_death) {
                    State::Infectious(Event {
                        s: StateEvent::Recovery,
                        p_hosp: self.p_hosp,
                        p_death: self.p_death,
                        t: now + State::get_time_normal(rng),
                    })
                } else {
                    State::Infectious(Event {
                        s: StateEvent::Hospitalization,
                        p_hosp: self.p_hosp,
                        p_death: self.p_death,
                        t: now + State::get_time_normal(rng),
                    })
                }
            }
            StateEvent::Hospitalization => {
                if rng.gen_bool(self.p_hosp) {
                    State::Hospitalized(Event {
                        s: StateEvent::Recovery,
                        p_hosp: self.p_hosp,
                        p_death: self.p_death,
                        t: now + State::get_time_normal(rng),
                    })
                } else {
                    State::Hospitalized(Event {
                        s: StateEvent::Death,
                        p_hosp: self.p_hosp,
                        p_death: self.p_death,
                        t: now + State::get_time_normal(rng),
                    })
                }
            }
            StateEvent::Death => State::Dead,
            StateEvent::Recovery => State::Recovered,
        }
    }
}

impl State {
    fn new(t0: AnyTime, p_hosp: f64, p_death: f64, rng: &mut XorShiftRng) -> Self {
        Self::Sane(Event {
            s: StateEvent::Exposition,
            p_hosp,
            p_death,
            t: t0 + Self::get_time_exp(rng),
        })
    }

    fn get_time_exp(rng: &mut XorShiftRng) -> geom::Duration {
        let normal = Exp::new(1.0).unwrap();
        Duration::seconds(normal.sample(rng))
    }

    fn get_time_normal(rng: &mut XorShiftRng) -> geom::Duration {
        let normal = Normal::new(10.0, 1.0).unwrap();
        Duration::seconds(normal.sample(rng))
    }

    // pub fn get_time(&self) -> Option<AnyTime> {
    //     match self {
    //         Self::Sane(ev) | Self::Exposed(ev) | Self::Infectious(ev) | Self::Hospitalized(ev) => {
    //             Some(ev.t)
    //         }
    //         Self::Recovered | Self::Dead => None,
    //     }
    // }

    pub fn set_time(mut self, new_time: AnyTime) -> Self {
        match self {
            Self::Sane(Event {
                s,
                p_hosp,
                p_death,
                t: _,
            }) => Self::Sane(Event {
                s,
                p_hosp,
                p_death,
                t: new_time,
            }),
            _ => unreachable!(),
        }
    }

    pub fn next_default(self, default: AnyTime, rng: &mut XorShiftRng) -> Option<Self> {
        // TODO: when #![feature(bindings_after_at)] reaches stable
        // rewrite this part with it
        match self {
            Self::Sane(ev) => Some(ev.next(default, rng)),
            Self::Exposed(ev) => Some(ev.next(default, rng)),
            Self::Infectious(ev) => Some(ev.next(default, rng)),
            Self::Hospitalized(ev) => Some(ev.next(default, rng)),
            Self::Recovered => None,
            Self::Dead => None,
        }
    }

    pub fn next(self, now: AnyTime, rng: &mut XorShiftRng) -> Option<Self> {
        // TODO: when #![feature(bindings_after_at)] reaches stable
        // rewrite this part with it
        match self {
            Self::Sane(ev) => {
                if ev.t <= now {
                    Some(ev.next(now, rng))
                } else {
                    Some(Self::Sane(ev))
                }
            }
            Self::Exposed(ev) => {
                if ev.t <= now {
                    Some(ev.next(now, rng))
                } else {
                    Some(Self::Exposed(ev))
                }
            }
            Self::Infectious(ev) => {
                if ev.t <= now {
                    Some(ev.next(now, rng))
                } else {
                    Some(Self::Infectious(ev))
                }
            }
            Self::Hospitalized(ev) => {
                if ev.t <= now {
                    Some(ev.next(now, rng))
                } else {
                    Some(Self::Hospitalized(ev))
                }
            }
            Self::Recovered => None,
            Self::Dead => None,
        }
    }
}

pub enum SEIR {
    Sane,
    Exposed,
    Infectious,
    Recovered,
}

impl SEIR {
    const T_INF: f64 = 3600.0 * 10.0; // TODO dummy values
    const T_INC: f64 = 3600.0; // TODO dummy values
    const R_0: f64 = 2.5;
    const S_RATIO: f64 = 0.985;
    const E_RATIO: f64 = 0.01;
    const I_RATIO: f64 = 0.05;
    const R_RATIO: f64 = 0.0;

    // TODO change that name it's bad
    pub fn get_transition_time_from(state: Self) -> Duration {
        match state {
            SEIR::Sane => Duration::seconds(SEIR::T_INF / SEIR::R_0),
            SEIR::Exposed => Duration::seconds(SEIR::T_INC),
            SEIR::Infectious => Duration::seconds(SEIR::T_INF),
            SEIR::Recovered => unreachable!(),
        }
    }

    // TODO ATM the sigma is simply the duration / 2. Maybe look that a bit more.
    // TODO also change that name it's bad
    pub fn get_transition_time_uncertainty_from(state: Self) -> Duration {
        match state {
            SEIR::Sane => Duration::seconds(SEIR::T_INF / SEIR::R_0 / 2.0),
            SEIR::Exposed => Duration::seconds(SEIR::T_INC / 2.0),
            SEIR::Infectious => Duration::seconds(SEIR::T_INF / 2.0),
            SEIR::Recovered => panic!("Impossible to transition from Recovered state"),
        }
    }

    pub fn get_initial_ratio(state: Self) -> f64 {
        match state {
            SEIR::Sane => SEIR::S_RATIO,
            SEIR::Exposed => SEIR::E_RATIO,
            SEIR::Infectious => SEIR::I_RATIO,
            SEIR::Recovered => SEIR::R_RATIO,
        }
    }
}
