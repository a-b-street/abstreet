//! A/B Street organizes data files [in a particular
//! way](https://a-b-street.github.io/docs/dev/data.html). This crate implements methods to
//! find files and (mostly) treat them the same way on native and web.

#![allow(clippy::type_complexity)]

#[macro_use]
extern crate anyhow;
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

#[cfg(not(target_arch = "wasm32"))]
mod download;
#[cfg(not(target_arch = "wasm32"))]
pub use download::*;

pub use abst_data::*;
pub use abst_paths::*;

mod abst_data;
mod abst_paths;
mod io;

/// An adapter for widgetry::Settings::read_svg to read SVGs using this crate's methods for finding
/// and reading files in different environments.
pub fn slurp_bytes(filename: &str) -> Vec<u8> {
    let path = path(filename);
    slurp_file(&path).unwrap_or_else(|_| panic!("Can't read {}", path))
}
