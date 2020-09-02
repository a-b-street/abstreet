#!/bin/bash

RUST_BACKTRACE=1 cargo run --bin importer --release --manifest-path importer/Cargo.toml --features scenarios -- $@;
