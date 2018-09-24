extern crate multimap;
extern crate serde;
extern crate serde_cbor;
extern crate serde_json;

mod abst_multimap;
mod io;

pub use abst_multimap::MultiMap;
pub use io::{
    deserialize_btreemap, deserialize_multimap, load_all_objects, read_binary, read_json,
    serialize_btreemap, serialize_multimap, to_json, write_binary, write_json,
};
