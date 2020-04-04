mod prob;

pub use prob::{proba_decaying_sigmoid, erf_distrib_bounded};
use geom::{Duration};

enum SEIR {
    Sane,
    Exposed,
    Infectious,
    Recovered,
}

impl SEIR {
    const T_INF: f64 = 3600.0 * 10.0; // TODO dummy values
    const T_INC: f64 = 3600.0; // TODO dummy values
    const R_0: f64 = 2.5;
    const S_RATIO: f64 = 0.01;
    const E_RATIO: f64 = SEIR::I_RATIO / 2.0;
    const I_RATIO: f64 = 0.01;
    const R_RATIO: f64 = 0.0;


    pub fn get_transition_time_from(state: Self) -> Duration {
        match state {
            SEIR::Sane => Duration::seconds(SEIR::T_INF / SEIR::R_0),
            SEIR::Exposed => Duration::seconds(SEIR::T_INC),
            SEIR::Infectious => Duration::seconds(SEIR::T_INF),
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