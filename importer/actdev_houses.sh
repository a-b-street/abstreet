#!/bin/bash
# This script procedurally generates houses for an actdev site that's already
# been imported, if the map doesn't seem to have houses mapped in OSM already.
# It's run manually once per site. It'd be better to incorporate this as a
# proper importer stage, but there's not an easy way to express that kind of
# task dependency yet.

CITY=$1
if [ "$CITY" == "" ]; then
	echo Missing args;
	exit 1;
fi

if cargo run --release --bin generate_houses -- --map=data/system/gb/$CITY/maps/center.bin --num_required=1000 --rng_seed=42 --out=data/input/gb/$CITY/procgen_houses.json; then
	# Update the importer config, and import again
	perl -pi -e "s#\"extra_buildings\": null#\"extra_buildings\": \"data/input/gb/$CITY/procgen_houses.json\"#" importer/config/gb/$CITY/cfg.json
	./import.sh --raw --map --city=gb/$CITY
else
	echo "$CITY already had enough houses"
fi
