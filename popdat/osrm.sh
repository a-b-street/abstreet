#!/bin/bash

cp ~/abstreet/data/input/gb/london/osm/greater-london-latest.osm.pbf input.osm.pbf
docker run -t -v "${PWD}:/data" osrm/osrm-backend osrm-extract -p /opt/car.lua /data/input.osm.pbf
docker run -t -v "${PWD}:/data" osrm/osrm-backend osrm-partition /data/input.osrm
docker run -t -v "${PWD}:/data" osrm/osrm-backend osrm-customize /data/input.osrm
docker run -t -i -p 5000:5000 -v "${PWD}:/data" osrm/osrm-backend osrm-routed --algorithm mld /data/input.osrm
