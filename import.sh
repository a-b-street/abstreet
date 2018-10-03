#!/bin/bash

set -e

function get_if_needed {
	if [ ! -f $2 ]; then
		wget $1 -O $2;
	fi
}

# Download raw data if needed.
mkdir -p data/input data/maps
# TODO Fill these out:
# http://download.bbbike.org/osm/bbbike/Seattle/
# https://gis-kingcounty.opendata.arcgis.com/datasets/king-county-parcels--parcel-area/geoservice
# https://data.seattle.gov/Transportation/Traffic-Signals/dr6d-ejex
# https://dds.cr.usgs.gov/srtm/version2_1/SRTM1/Region_01/N47W122.hgt.zip
# https://data.seattle.gov/api/views/77ms-czxg/rows.json?accessType=DOWNLOAD
# Seattle bounding box is -b=-122.4416,47.5793,-122.2421,47.7155
# https://metro.kingcounty.gov/GTFS/google_transit_2018_18_08.zip

ELEVATION=../data/input/N47W122.hgt
PARCELS_KML=../data/input/King_County_Parcels__parcel_area.kml
TRAFFIC_SIGNALS=../data/input/TrafficSignals.shp
GTFS=../data/input/google_transit_2018_18_08

SMALL_OSM=../data/input/tiny_montlake.osm
MEDIUM_OSM=../data/input/montlake.osm
LARGE_OSM=../data/input/small_seattle.osm
HUGE_OSM=../data/input/seattle.osm

if [ ! -f data/maps/seattle_parcels.abst ]; then
	cd kml
	time cargo run --release $PARCELS_KML ../data/maps/seattle_parcels.abst
	cd ..
fi

COMMON="--elevation=$ELEVATION --traffic_signals=$TRAFFIC_SIGNALS --parcels=../data/seattle_parcels.abst --gtfs=$GTFS"
cd convert_osm
time cargo run --release -- --osm=$SMALL_OSM $COMMON --output=../data/maps/small.abst
time cargo run --release -- --osm=$MEDIUM_OSM $COMMON --output=../data/maps/medium.abst
time cargo run --release -- --osm=$LARGE_OSM $COMMON --output=../data/maps/large.abst
time cargo run --release -- --osm=$HUGE_OSM $COMMON --output=../data/maps/huge.abst
