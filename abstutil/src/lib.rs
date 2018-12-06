#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate serde_derive;

mod clone;
mod collections;
mod error;
mod io;
mod logs;
mod notes;
mod random;
mod time;

pub use crate::clone::Cloneable;
pub use crate::collections::{wraparound_get, MultiMap};
pub use crate::error::Error;
pub use crate::io::{
    deserialize_btreemap, deserialize_multimap, list_all_objects, load_all_objects, read_binary,
    read_json, save_object, serialize_btreemap, serialize_multimap, to_json, write_binary,
    write_json, FileWithProgress,
};
pub use crate::logs::{format_log_record, LogAdapter};
pub use crate::notes::note;
pub use crate::random::{fork_rng, WeightedUsizeChoice};
pub use crate::time::{elapsed_seconds, Timer};

const PROGRESS_FREQUENCY_SECONDS: f64 = 0.2;
