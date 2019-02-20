mod clone;
mod collections;
mod error;
mod io;
mod logs;
mod notes;
mod random;
mod time;

pub use crate::clone::Cloneable;
pub use crate::collections::{retain_btreemap, wraparound_get, MultiMap};
pub use crate::error::Error;
pub use crate::io::{
    deserialize_btreemap, deserialize_multimap, find_next_file, find_prev_file, list_all_objects,
    load_all_objects, read_binary, read_json, save_object, serialize_btreemap, serialize_multimap,
    to_json, write_binary, write_json, FileWithProgress,
};
pub use crate::logs::Warn;
pub use crate::notes::note;
pub use crate::random::{fork_rng, WeightedUsizeChoice};
pub use crate::time::{elapsed_seconds, prettyprint_usize, MeasureMemory, Profiler, Timer};

const PROGRESS_FREQUENCY_SECONDS: f64 = 0.2;
