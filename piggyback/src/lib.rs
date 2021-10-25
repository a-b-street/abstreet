#[macro_use]
extern crate log;

#[cfg(target_arch = "wasm32")]
mod piggyback;

#[cfg(target_arch = "wasm32")]
pub use piggyback::*;

#[cfg(not(target_arch = "wasm32"))]
pub fn dummy() {
    info!("Just avoiding an unused warning");
}
