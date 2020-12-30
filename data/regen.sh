#!/bin/bash

set -e

rm -fv data/system/seattle/maps/huge_seattle.bin data/input/raw_maps/huge_seattle.bin data/input/seattle/popdat.bin

./import.sh --raw --map --scenario
./import.sh --raw --map --city=bellevue
./import.sh --raw --map --city=berlin
./import.sh --raw --map --city=krakow
./import.sh --raw --map --city=leeds
./import.sh --raw --map --city=london
./import.sh --raw --map --city_overview --city=nyc
./import.sh --raw --map --city_overview --city=paris
./import.sh --raw --map --city_overview --city=salzburg
./import.sh --raw --map --city=tel_aviv
./import.sh --raw --map --city=xian

cargo run --release --bin game -- --prebake

cargo run --release --bin tests
