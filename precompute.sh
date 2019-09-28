#!/bin/bash

set -e

release_mode=""
psrc_scenarios=""
for arg in "$@"; do
	if [ "$arg" == "--release" ]; then
		release_mode="--release";
	elif [ "$arg" == "--disable_psrc_scenarios" ]; then
		psrc_scenarios="--disable_psrc_scenarios";
	else
		# Just recompute a single map.
		cd precompute;
		RUST_BACKTRACE=1 cargo run $release_mode ../data/raw_maps/$arg.bin $psrc_scenarios;
		cd ..;
		exit;
	fi
done

mkdir -p data/maps/

# Need this first
if [ ! -f data/shapes/popdat.bin ]; then
	# We probably don't have this map yet.
	if [ ! -f data/maps/huge_seattle.bin ]; then
		cd precompute;
		RUST_BACKTRACE=1 cargo run --release ../data/raw_maps/huge_seattle.bin --disable_psrc_scenarios;
		cd ..;
	fi

	cd popdat;
	cargo run --release;
	cd ..;
fi

for map_path in `ls data/raw_maps/`; do
	map=`basename $map_path .bin`;
	echo "Precomputing $map";
	cd precompute;
	RUST_BACKTRACE=1 cargo run $release_mode ../data/raw_maps/$map.bin $psrc_scenarios;
	cd ..;
done
