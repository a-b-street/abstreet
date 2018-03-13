#!/bin/bash
#
# Copyright 2018 Google LLC
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#      http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

set -e

# Download raw data if needed.
mkdir -p data/input
# TODO Fill these out:
# http://download.bbbike.org/osm/bbbike/Seattle/
# https://gis-kingcounty.opendata.arcgis.com/datasets/king-county-parcels--parcel-area/geoservice
# https://data.seattle.gov/Transportation/Traffic-Signals/dr6d-ejex
# https://dds.cr.usgs.gov/srtm/version2_1/SRTM1/Region_01/N47W122.hgt.zip
# Seattle bounding box is -b=-122.4416,47.5793,-122.2421,47.7155

ELEVATION=../data/input/N47W122.hgt
PARCELS_KML=../data/input/King_County_Parcels__parcel_area.kml
TRAFFIC_SIGNALS=../data/inputTrafficSignals.shp

SMALL_OSM=../data/input/tiny_montlake.osm
MEDIUM_OSM=../data/input/montlake.osm
LARGE_OSM=../data/input/small_seattle.osm
HUGE_OSM=../data/input/seattle.osm

if [ ! -f data/seattle_parcels.abst ]; then
  cd kml
  time cargo run --release $PARCELS_KML ../data/seattle_parcels.abst
  cd ..
fi

COMMON="--elevation=$ELEVATION --traffic_signals=$TRAFFIC_SIGNALS --parcels=../data/seattle_parcels.abst"
cd convert_osm
time cargo run --release -- --osm=$SMALL_OSM $COMMON --output=../data/small.abst
time cargo run --release -- --osm=$MEDIUM_OSM $COMMON --output=../data/medium.abst
time cargo run --release -- --osm=$LARGE_OSM $COMMON --output=../data/large.abst
time cargo run --release -- --osm=$HUGE_OSM $COMMON --output=../data/huge.abst
