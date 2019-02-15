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

rm -rf data/neighborhoods/$name data/maps/${name}_*.abst;

cd convert_osm;
RUST_BACKTRACE=1 cargo run $release_mode -- \
	--osm=../data/input/$name.osm \
	--elevation=../data/input/N47W122.hgt \
	--traffic_signals=../data/input/traffic_signals.kml \
	--neighborhoods=../data/input/neighborhoods.geojson \
	--clip=../data/polygons/$name.poly \
	--output=../data/raw_maps/$name.abst

cd ../precompute;
RUST_BACKTRACE=1 cargo run $release_mode ../data/raw_maps/$name.abst --edits_name=no_edits;
