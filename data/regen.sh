#!/bin/bash

set -e

rm -fv data/system/maps/huge_seattle.bin data/input/raw_maps/huge_seattle.bin data/input/seattle/popdat.bin

./import.sh --raw --map --scenario
./import.sh --raw --map --city=berlin
./import.sh --raw --map --city=krakow
./import.sh --raw --map --city=london
./import.sh --raw --map --city=tel_aviv
./import.sh --raw --map --city=xian

cargo run --release --bin game -- --prebake
cargo run --release --bin game -- --smoketest
cargo run --release --bin game -- --check_proposals
