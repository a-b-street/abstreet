/// On native: intercept messages using the `log` crate and print them to STDOUT. Contains
/// special handling to filter/throttle spammy messages from `fast_paths` and `hyper`.
///
/// On web: Just use console_log.
pub fn setup() {
    #[cfg(target_arch = "wasm32")]
    {
        console_log::init_with_level(log::Level::Info).unwrap();
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        use env_logger::{Builder, Env};
        Builder::from_env(Env::default().default_filter_or("info")).init();
    }
}
