#!/bin/bash

# This tool needs https://wiki.openstreetmap.org/wiki/Osmupdate#Download
# This is just for Seattle

set -e

# From http://download.geofabrik.de/north-america/us/washington.html
curl -L -O http://download.geofabrik.de/north-america/us/washington-latest.osm.pbf
# TODO Ideally limit update size with a clipping polygon, but it clips too
# aggressively for the huge_seattle map. I guess we'd need an even bigger
# boundary for that.
osmupdate -v washington-latest.osm.pbf updated_wa.osm.pbf
