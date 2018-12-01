#[macro_use]
extern crate lazy_static;
extern crate log;
extern crate multimap;
extern crate rand;
extern crate serde;
extern crate serde_cbor;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate yansi;

mod clone;
mod collections;
mod error;
mod io;
mod logs;
mod notes;
mod random;
mod time;

pub use clone::Cloneable;
pub use collections::{wraparound_get, MultiMap};
pub use error::Error;
pub use io::{
    deserialize_btreemap, deserialize_multimap, list_all_objects, load_all_objects, read_binary,
    read_json, save_object, serialize_btreemap, serialize_multimap, to_json, write_binary,
    write_json, FileWithProgress,
};
pub use logs::{format_log_record, LogAdapter};
pub use notes::note;
pub use random::{fork_rng, WeightedUsizeChoice};
pub use time::{elapsed_seconds, Timer};

const PROGRESS_FREQUENCY_SECONDS: f64 = 0.2;
