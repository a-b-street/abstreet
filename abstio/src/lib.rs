//! A/B Street organizes data files [in a particular
//! way](https://dabreegster.github.io/abstreet/dev/data.html). This crate implements methods to
//! find files and (mostly) treat them the same way on native and web.

#[macro_use]
extern crate log;

#[cfg(not(target_arch = "wasm32"))]
mod io_native;
#[cfg(not(target_arch = "wasm32"))]
pub use io_native::*;
#[cfg(target_arch = "wasm32")]
mod io_web;
#[cfg(target_arch = "wasm32")]
pub use io_web::*;

pub use abst_data::*;
pub use abst_paths::*;

mod abst_data;
mod abst_paths;
mod io;
