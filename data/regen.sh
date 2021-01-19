#!/bin/bash

set -e

rm -fv data/system/seattle/maps/huge_seattle.bin data/input/raw_maps/huge_seattle.bin data/input/seattle/popdat.bin

./import.sh --regen_all

cargo run --release --bin game -- --prebake

cargo run --release --bin tests
