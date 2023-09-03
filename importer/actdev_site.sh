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
mkdir -p importer/config/gb/$CITY
wget https://raw.githubusercontent.com/cyipt/actdev/main/data-small/$SITE/small-study-area.geojson
mv small-study-area.geojson importer/config/gb/$CITY/center.geojson

wget https://raw.githubusercontent.com/cyipt/actdev/main/data-small/$SITE/site.geojson -O data/system/study_areas/$SITE.geojson

./import.sh --raw --map --city=gb/$CITY

# Note we once ran 'generate-houses'; see version history for context

./importer/actdev_scenario.sh $CITY

echo "You have to manually update .gitignore, map_gui/src/tools/mod.rs, release/deploy_actdev.sh"
echo "And after uploading, probably want to: cargo run --bin updater -- opt-into-all > data/player/data.json"
