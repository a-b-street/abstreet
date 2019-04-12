#!/bin/bash

set -e

release_mode=""
for arg in "$@"; do
	if [ "$arg" == "--release" ]; then
		release_mode="--release";
	else
		echo "Unknown argument $arg";
		exit 1;
	fi
done

mkdir -p data/maps/

# Re-export all synthetic maps from scratch. Do this before the other loop,
# since the raw_map might be stale.
cd precompute;
for path in `ls ../data/synthetic_maps/*`; do
	RUST_BACKTRACE=1 cargo run $release_mode $path;
done
cd ..;

for map_path in `ls data/raw_maps/`; do
	map=`basename $map_path .abst`;
	echo "Precomputing $map";
	cd precompute;
	RUST_BACKTRACE=1 cargo run $release_mode ../data/raw_maps/$map.abst;
	cd ..;
done
