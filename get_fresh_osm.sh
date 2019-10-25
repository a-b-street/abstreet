#!/bin/bash

set -e

# From http://download.geofabrik.de/north-america/us/washington.html
wget http://download.geofabrik.de/north-america/us/washington-latest.osm.pbf
osmupdate -v washington-latest.osm.pbf updated_wa.osm.pbf
rm -fv washington-latest.osm.pbf data/input/*.osm
osmosis --read-pbf updated_wa.osm.pbf --bounding-polygon file=data/polygons/huge_seattle.poly completeWays=true --write-xml data/input/Seattle.osm
rm -fv updated_wa.osm.pbf
# Then run import.sh
