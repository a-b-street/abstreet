use std::sync::Once;

static SETUP: Once = Once::new();

/// ## On native: uses env_log
///
/// You can adjust the log level without recompiling with the RUST_LOG env variable.
///
/// ```skip
/// RUST_LOG=debug cargo run --bin game
/// ```
///
/// This can be done on a per lib basis:
///
/// ```skip
/// RUST_LOG=my_lib=debug cargo run --bin game
/// ```
///
/// Or a module-by-module basis:
///
/// ```skip
/// RUST_LOG=my_lib::module=debug cargo run --bin game
/// ```
///
/// You can mix and match:
///
/// ```skip
/// # error logging by default, except the foo:bar module at debug level
/// # and the entire baz crate at info level
/// RUST_LOG=error,foo::bar=debug,baz=info cargo run --bin game
/// ```
///
/// For some special cases, you might want to use regex matching by specifying a pattern with the
/// "/":
///
/// ```skip
/// # only log once every 10k
/// RUST_LOG="fast_paths=debug/contracted node [0-9]+0000 " mike import_la
/// ```
///
/// ## On web: uses console_log
///
/// Can be called multiple times
pub fn setup() {
    SETUP.call_once(|| {
        #[cfg(target_arch = "wasm32")]
        {
            console_log::init_with_level(log::Level::Info).unwrap();
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            use env_logger::{Builder, Env};
            Builder::from_env(Env::default().default_filter_or("info")).init();
        }
    });
}
