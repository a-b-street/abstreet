#!/bin/bash
# TODO It's probably more useful to now manually run commands from this file.

set -e

rm -fv data/input/us/seattle/raw_maps/huge_seattle.bin data/system/us/seattle/maps/huge_seattle.bin data/input/us/seattle/popdat.bin

RUST_BACKTRACE=1 cargo run --release --bin cli --features importer/scenarios -- regenerate-everything
# Or more efficiently:
# ./target/release/cli regenerate-everything-externally

# If a map changes that has external JSON scenarios, enable this!
# importer/external_scenarios.sh

RUST_BACKTRACE=1 cargo run --release --bin game -- --prebake

RUST_BACKTRACE=1 cargo run --release --bin tests
