#!/bin/bash
# This script imports a site from
# https://github.com/cyipt/actdev/tree/main/data-small as a new city and map.
# It follows https://a-b-street.github.io/docs/howto/new_city.html.

set -e

# This should be a directory name from https://github.com/cyipt/actdev/tree/main/data-small
SITE=$1
if [ "$SITE" == "" ]; then
	echo Missing args;
	exit 1;
fi
CITY=${SITE/-/_}

cp -Rv importer/config/leeds importer/config/$CITY
perl -pi -e "s#\"separate_cycleways\": false#\"separate_cycleways\": true#" importer/config/$CITY/cfg.json
rm -fv importer/config/$CITY/*.poly
wget https://raw.githubusercontent.com/cyipt/actdev/main/data-small/$SITE/small-study-area.geojson
cargo run --bin geojson_to_osmosis < small-study-area.geojson
rm -fv small-study-area.geojson
mv boundary0.poly importer/config/$CITY/center.poly
GEOFABRIK=`cargo run --bin pick_geofabrik importer/config/$CITY/center.poly`
echo "Geofabrik URL is $GEOFABRIK"
perl -pi -e "s#\"osm_url\": \".*\"#\"osm_url\": \"$GEOFABRIK\"#" importer/config/$CITY/cfg.json

wget https://raw.githubusercontent.com/cyipt/actdev/main/data-small/$SITE/site.geojson -O data/system/study_areas/$SITE.geojson

./import.sh --raw --map --city=$CITY

echo "You have to manually update .gitignore, map_gui/src/tools/mod.rs, release/deploy_actdev.sh"
echo "You might need to procedurally generate houses."
