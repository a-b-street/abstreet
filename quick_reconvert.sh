#!/bin/bash

set -e

release_mode=""
name=""

for arg in "$@"; do
	if [ "$arg" == "--release" ]; then
		release_mode="--release";
	else
		name=$arg;
	fi
done
if [ "$name" == "" ]; then
	echo "Pass a map name";
	exit;
fi

# TODO Argh, copied code! Need to detangle all the scripts.

rm -rf data/neighborhoods/$name;

cd convert_osm;
RUST_BACKTRACE=1 cargo run $release_mode -- \
	--osm=../data/input/$name.osm \
	--clip=../data/polygons/$name.poly \
	--output=../data/raw_maps/$name.bin

cd ../precompute;
# TODO Should be --disable_psrc_scenarios=true, but structopt and bools are weird...
RUST_BACKTRACE=1 cargo run $release_mode ../data/raw_maps/$name.bin true;
