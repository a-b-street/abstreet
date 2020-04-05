// probability after a time t at a given rate
pub fn proba_decaying_sigmoid(time: f64, rate: f64) -> f64 {
    if time < 0.0 {
        panic!("Error the time must be always be positive but was {}", time);
    }
    let prob = 1.0 - (-time * rate).exp();
    prob
}

// goind from -infinity to t
// used for gaussian distribution probability from -inf to t
#[allow(dead_code)]
pub fn erf_distrib(t: f64, mu: f64, sigma: f64) -> f64 {
    if sigma < 0.0 {
        panic!("Error sigma must be always be positive but was {}", sigma);
    }
    0.5 - 0.5 * libm::erf((-t + mu) / (f64::sqrt(2.0) * sigma))
}

// going from t1 to t1
// used for gaussian distribution probability from t0 to t1
// t1 >= t0
pub fn erf_distrib_bounded(t0: f64, t1: f64, mu: f64, sigma: f64) -> f64 {
    if t1 < t0 {
        panic!("Error t0 = {} < and t1 = {}. t1 must be larger than t0.", t0, t1);
    }
    if sigma < 0.0 {
        panic!("Error sigma must be always be positive but was {}", sigma);
    }

    0.5 * libm::erf((-t0 + mu) / (f64::sqrt(2.0) * sigma))
        - 0.5 * libm::erf((-t1 + mu) / (f64::sqrt(2.0) * sigma))
}


#[cfg(test)]
mod tests {
    use super::*;
    use rand::prelude::*;

    fn prob_range(p: f64) -> bool {
        p >= 0.0 && p <= 1.0
    }

    #[test]
    #[should_panic]
    fn test_time_sigmoid() {
        proba_decaying_sigmoid(-1.0, 1.0);
    }

    #[test]
    fn test_range_sigmoid() {
        let range = 1000.0;
        let max = 100;
        let mut rng = rand::thread_rng();
        for _ in 0..max {
            prob_range(proba_decaying_sigmoid(rng.gen::<f64>() * range, rng.gen::<f64>() * range));
        }
    }

    #[test]
    fn test_range_erf_distrib() {
        let range = 1000.0;
        let max = 100;
        let mut rng = rand::thread_rng();
        for _ in 0..max {
            prob_range(erf_distrib(rng.gen::<f64>() * range, rng.gen::<f64>() * range, rng.gen::<f64>() * range));
        }
    }


    #[test]
    fn test_range_erf_distrib_bounded() {
        let range = 1000.0;
        let max = 100;
        let mut rng = rand::thread_rng();
        for _ in 0..max {
            let t0 = rng.gen::<f64>() * range;
            let t1 = t0 + 1.0;
            prob_range(erf_distrib_bounded(t0, t1, rng.gen::<f64>() * range, rng.gen::<f64>() * range));
        }
    }

    #[test]
    #[should_panic]
    fn test_range_erf_bounded() {
        erf_distrib_bounded(-1.0, -2.0, 1.0, 1.0);
    }

    #[test]
    #[should_panic]
    fn test_sigma_erf_distrib() {
        erf_distrib(1.0, 1.0, -1.0);
    }

    #[test]
    #[should_panic]
    fn test_sigma_erf_distrib_bounded() {
        erf_distrib_bounded(1.0, 2.0, 1.0, -1.0);
    }

}
