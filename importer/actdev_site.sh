#!/bin/bash
# This script imports a site from
# https://github.com/cyipt/actdev/tree/main/data-small as a new city and map.
# It follows https://a-b-street.github.io/docs/howto/new_city.html.
#
# Here's a sample...
#
# ./importer/actdev_site.sh allerton-bywater allerton_bywater center leeds   # Reuse the leeds osm
# ./importer/actdev_site.sh bailrigg lancaster bailrigg lancashire
# ./importer/actdev_site.sh didcot harwell didcot oxfordshire
# ./importer/actdev_site.sh ebbsfleet dartford ebbsfleet kent
# ./importer/actdev_site.sh handforth poynton handforth greater-manchester
# ./importer/actdev_site.sh long-marston stratford_upon_avon long_marston warwickshire

set -e

SITE=$1
CITY=$2
MAP=$3
GEOFABRIK=$4
if [ "$SITE" == "" ] || [ "$CITY" == "" ] || [ "$MAP" == "" ] || [ "$GEOFABRIK" == "" ]; then
	echo Missing args;
	exit 1;
fi

cp -Rv importer/config/cambridge importer/config/$CITY
rm -fv importer/config/$CITY/*.poly
wget https://raw.githubusercontent.com/cyipt/actdev/main/data-small/$SITE/small-study-area.geojson
cargo run --bin geojson_to_osmosis < small-study-area.geojson
rm -fv small-study-area.geojson
mv boundary0.poly importer/config/$CITY/$MAP.poly
perl -pi -e "s/cambridgeshire/$GEOFABRIK/g" importer/config/$CITY/cfg.json

#./import.sh --raw --map --city=$CITY

echo "You have to manually update .gitignore, importer/src/main.rs, map_gui/src/tools/mod.rs"
