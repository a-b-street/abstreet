//! The contents of this crate need to be organized better:
//!
//! - Timer (a mix of logging, profiling, and even parallel execution)
//! - true utility functions (collections, prettyprinting, CLI parsing

// I'm not generally a fan of wildcard exports, but they're more maintable here.
pub use crate::serde::*;
pub use cli::*;
pub use collections::*;
pub use logger::*;
pub use process::*;
pub use time::*;
pub use utils::*;

mod cli;
mod collections;
mod logger;
mod process;
mod serde;
pub mod time;
mod utils;

pub const PROGRESS_FREQUENCY_SECONDS: f64 = 0.2;
