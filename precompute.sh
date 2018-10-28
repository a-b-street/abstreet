#!/bin/bash

set -e

mkdir -p data/maps/

for map_path in `ls data/raw_maps/`; do
	map=`basename $map_path .abst`;
	if [ -e data/edits/$map ]; then
		# Line based iteration, since filenames might have spaces
		ls data/edits/$map/ | while read edit_path
		do
			edits=`basename "$edit_path" .json`;
			echo "Precomputing $map with $edits";
			cd precompute;
			time cargo run --release ../data/raw_maps/$map.abst --edits_name="$edits";
			cd ..;
		done
	fi
done
