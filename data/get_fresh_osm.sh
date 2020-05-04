#!/bin/bash

# This tool needs https://wiki.openstreetmap.org/wiki/Osmupdate#Download
# This is just for Seattle

set -e

# From http://download.geofabrik.de/north-america/us/washington.html
curl -L -O http://download.geofabrik.de/north-america/us/washington-latest.osm.pbf
osmupdate -v washington-latest.osm.pbf updated_wa.osm.pbf
