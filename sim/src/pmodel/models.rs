struct SEIR;

impl SEIR {
    const T_INF: f64 = 3600.0 * 10.0; // TODO dummy values
    const T_INC: f64 = 3600.0; // TODO dummy values
    const R_0: f64 = 2.5;
    const I_RATIO: f64 = 0.01;
    const E_RATIO: f64 = SEIR::I_RATIO / 2.0;
}