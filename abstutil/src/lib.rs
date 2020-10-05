// The contents of this crate need to be organized better:
//
// - Timer (a mix of logging, profiling, and even parallel execution)
// - IO utilities, some of which have web equivalents using include_dir
// - A/B Street-specific filesystem paths
// - true utility functions (collections, prettyprinting, CLI parsing

mod abst_paths;
mod cli;
mod collections;
mod io;
mod serde;
mod time;
mod utils;

// I'm not generally a fan of wildcard exports, but they're more maintable here.
pub use crate::serde::*;
pub use abst_paths::*;
pub use cli::*;
pub use collections::*;
pub use io::*;
pub use time::*;
pub use utils::*;

const PROGRESS_FREQUENCY_SECONDS: f64 = 0.2;
