#!/bin/bash

set -e

release_mode=""
for arg in "$@"; do
	if [ "$arg" == "--release" ]; then
		release_mode="--release";
	else
		# Just recompute a single map.
		cd precompute;
		RUST_BACKTRACE=1 cargo run $release_mode ../data/raw_maps/$arg.bin;
		cd ..;
		exit;
	fi
done

mkdir -p data/maps/

for map_path in `ls data/raw_maps/`; do
	map=`basename $map_path .bin`;
	echo "Precomputing $map";
	cd precompute;
	RUST_BACKTRACE=1 cargo run $release_mode ../data/raw_maps/$map.bin;
	cd ..;
done
