mod prob;

pub use prob::{proba_decaying_sigmoid, erf_distrib_bounded};
use geom::{Duration};

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
    const E_RATIO: f64 = 0.005;
    const I_RATIO: f64 = 0.01;
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
            SEIR::Recovered => {
                panic!("Impossible to transition from Recovered state")
            }
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
