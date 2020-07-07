#!/bin/bash

RUST_BACKTRACE=1 cargo run --release --manifest-path importer/Cargo.toml --features scenarios -- $@;
