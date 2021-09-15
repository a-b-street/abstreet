#!/bin/bash
# This script imports a site from
# https://github.com/cyipt/actdev/tree/main/data-small as a new city.

set -e

# This should be a directory name from https://github.com/cyipt/actdev/tree/main/data-small
SITE=$1
if [ "$SITE" == "" ]; then
	echo Missing args;
	exit 1;
fi
CITY=`echo $SITE | sed -r 's/-/_/g'`

# Follow https://a-b-street.github.io/docs/user/new_city.html and import as a new city.
cp -Rv importer/config/gb/leeds importer/config/gb/$CITY
rm -fv importer/config/gb/$CITY/*.poly
wget https://raw.githubusercontent.com/cyipt/actdev/main/data-small/$SITE/small-study-area.geojson
cargo run --bin cli -- geojson-to-osmosis small-study-area.geojson
rm -fv small-study-area.geojson
mv boundary0.poly importer/config/gb/$CITY/center.poly
GEOFABRIK=`cargo run --bin pick_geofabrik importer/config/gb/$CITY/center.poly`
echo "Geofabrik URL is $GEOFABRIK"
perl -pi -e "s#\"osm_url\": \".*\"#\"osm_url\": \"$GEOFABRIK\"#" importer/config/gb/$CITY/cfg.json

wget https://raw.githubusercontent.com/cyipt/actdev/main/data-small/$SITE/site.geojson -O data/system/study_areas/$SITE.geojson

./import.sh --raw --map --city=gb/$CITY

# Procedurally generate houses, if needed
if cargo run --release --bin cli -- generate-houses --map=data/system/gb/$CITY/maps/center.bin --num_required=1000 --rng_seed=42 --out=data/input/gb/$CITY/procgen_houses.json; then
	# Update the importer config, and import again
	perl -pi -e "s#\"extra_buildings\": null#\"extra_buildings\": \"data/input/gb/$CITY/procgen_houses.json\"#" importer/config/gb/$CITY/cfg.json
	./import.sh --raw --map --city=gb/$CITY
else
	echo "$CITY already had enough houses"
fi

./importer/actdev_scenario.sh $CITY

echo "You have to manually update .gitignore, map_gui/src/tools/mod.rs, release/deploy_actdev.sh"
echo "And after uploading, probably want to: cargo run --bin updater -- --opt-into-all > data/player/data.json"
