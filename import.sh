#!/bin/bash

cd importer;
RUST_BACKTRACE=1 cargo run --release -- $@;
cd ..;
