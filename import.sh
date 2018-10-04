#!/bin/bash

set -e

# First prepare input.

function get_if_needed {
	if [ ! -f $2 ]; then
		wget $1 -O $2;
	fi
}

mkdir -p data/input data/maps

# TODO refactor a variant for .zips?
if [ ! -f data/input/N47W122.hgt ]; then
	get_if_needed https://dds.cr.usgs.gov/srtm/version2_1/SRTM1/Region_01/N47W122.hgt.zip data/input/N47W122.hgt.zip;
	unzip -d data/input data/input/N47W122.hgt.zip;
	rm -f data/input/N47W122.hgt.zip;
fi

if [ ! -d data/input/google_transit_2018_18_08/ ]; then
	get_if_needed https://metro.kingcounty.gov/GTFS/google_transit_2018_18_08.zip data/input/google_transit_2018_18_08.zip;
	unzip -d data/input/google_transit_2018_18_08 data/input/google_transit_2018_18_08.zip;
	rm -f data/input/google_transit_2018_18_08.zip;
fi

if [ ! -f data/input/TrafficSignals.shp ]; then
	get_if_needed https://data.seattle.gov/download/dr6d-ejex/application%2Fzip data/input/TrafficSignals.shp.zip;
	unzip -d data/input data/input/TrafficSignals.shp.zip;
	mv data/input/Traffic\ Signals/WGS84/TrafficSignals.shp data/input;
	rm -rf data/input/Traffic\ Signals data/input/TrafficSignals.shp.zip;
fi

# From https://gis-kingcounty.opendata.arcgis.com/datasets/king-county-parcels--parcel-area/geoservice
get_if_needed https://opendata.arcgis.com/datasets/8058a0c540434dadbe3ea0ade6565143_439.kml data/input/King_County_Parcels__parcel_area.kml;

if [ ! -f data/input/Seattle.osm ]; then
	get_if_needed http://download.bbbike.org/osm/bbbike/Seattle/Seattle.osm.gz data/input/Seattle.osm.gz;
	gunzip data/input/Seattle.osm.gz;
fi

# TODO could be more declarative... list bbox or polygon, then name of slice
if [ ! -f data/input/small_seattle.osm ]; then
	osmosis --read-xml enableDateParsing=no file=data/input/Seattle.osm --bounding-box left=-122.4416 bottom=47.5793 right=-122.2421 top=47.7155 --write-xml data/input/small_seattle.osm
fi

if [ ! -f data/input/montlake.osm ]; then
	osmosis --read-xml enableDateParsing=no file=data/input/Seattle.osm --bounding-box left=-122.3218 bottom=47.6323 right=-122.2985 top=47.6475 --write-xml data/input/montlake.osm
fi

ELEVATION=../data/input/N47W122.hgt
PARCELS_KML=../data/input/King_County_Parcels__parcel_area.kml
TRAFFIC_SIGNALS=../data/input/TrafficSignals.shp
GTFS=../data/input/google_transit_2018_18_08

if [ ! -f data/seattle_parcels.abst ]; then
	cd kml
	time cargo run --release $PARCELS_KML ../data/seattle_parcels.abst
	cd ..
fi

COMMON="--elevation=$ELEVATION --traffic_signals=$TRAFFIC_SIGNALS --parcels=../data/seattle_parcels.abst --gtfs=$GTFS"
cd convert_osm
time cargo run --release -- --osm=../data/input/montlake.osm $COMMON --output=../data/maps/montlake.abst
time cargo run --release -- --osm=../data/input/small_seattle.osm $COMMON --output=../data/maps/small_seattle.abst
