#!/bin/bash

# If converting one map fails, keep converting other maps.
#set -e

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

for map_path in `ls data/raw_maps/`; do
	map=`basename $map_path .abst`;
	echo "Precomputing $map with no_edits";
	cd precompute;
	cargo run $release_mode ../data/raw_maps/$map.abst --edits_name=no_edits;
	cd ..;

	if [ -e data/edits/$map ]; then
		# Line based iteration, since filenames might have spaces
		ls data/edits/$map/ | while read edit_path
		do
			edits=`basename "$edit_path" .json`;
			echo "Precomputing $map with $edits";
			cd precompute;
			cargo run $release_mode ../data/raw_maps/$map.abst --edits_name="$edits";
			cd ..;
		done
	fi
done
