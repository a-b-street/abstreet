#!/bin/bash

set -e

# First prepare input.

function get_if_needed {
	if [ ! -f $2 ]; then
		wget $1 -O $2;
	fi
}

mkdir -p data/input data/raw_maps

# TODO refactor a variant for .zips?
if [ ! -f data/input/N47W122.hgt ]; then
	get_if_needed \
		https://dds.cr.usgs.gov/srtm/version2_1/SRTM1/Region_01/N47W122.hgt.zip \
		data/input/N47W122.hgt.zip;
	unzip -d data/input data/input/N47W122.hgt.zip;
	rm -f data/input/N47W122.hgt.zip;
fi

if [ ! -d data/input/google_transit_2018_18_08/ ]; then
	get_if_needed \
		https://metro.kingcounty.gov/GTFS/google_transit_2018_18_08.zip \
		data/input/google_transit_2018_18_08.zip;
	unzip -d data/input/google_transit_2018_18_08 data/input/google_transit_2018_18_08.zip;
	rm -f data/input/google_transit_2018_18_08.zip;
fi

if [ ! -f data/input/TrafficSignals.shp ]; then
	get_if_needed \
		https://data.seattle.gov/download/dr6d-ejex/application%2Fzip \
		data/input/TrafficSignals.shp.zip;
	unzip -d data/input data/input/TrafficSignals.shp.zip;
	mv data/input/Traffic\ Signals/WGS84/TrafficSignals.shp data/input;
	rm -rf data/input/Traffic\ Signals data/input/TrafficSignals.shp.zip;
fi

if [ ! -f data/input/neighborhoods.geojson ]; then
	# https://data.seattle.gov/dataset/Neighborhoods/2mbt-aqqx in GeoJSON, not SHP
	get_if_needed \
		https://github.com/seattleio/seattle-boundaries-data/raw/master/data/neighborhoods.geojson \
		data/input/neighborhoods.geojson;
fi

if [ ! -f data/input/Seattle.osm ]; then
	get_if_needed \
		http://download.bbbike.org/osm/bbbike/Seattle/Seattle.osm.gz \
		data/input/Seattle.osm.gz;
	gunzip data/input/Seattle.osm.gz;
fi

for poly in `ls data/polygons/`; do
	name=`basename -s .poly $poly`;
	if [ ! -f data/input/$name.osm ]; then
		osmosis \
			--read-xml enableDateParsing=no file=data/input/Seattle.osm \
			--bounding-polygon file=data/polygons/$name.poly completeWays=true \
			--write-xml data/input/$name.osm
	fi
done

if [ ! -f data/shapes/parcels ]; then
	# From https://gis-kingcounty.opendata.arcgis.com/datasets/king-county-parcels--parcel-area/geoservice
	# TODO This isn't a direct link
	#get_if_needed https://opendata.arcgis.com/datasets/8058a0c540434dadbe3ea0ade6565143_439.kml data/input/King_County_Parcels__parcel_area.kml;

	cd kml
	time cargo run --release -- \
		--input=../data/input/King_County_Parcels__parcel_area.kml \
		--output=../data/shapes/parcels
	cd ..
fi

cd convert_osm
for poly in `ls ../data/polygons/`; do
	name=`basename -s .poly $poly`;
	cargo run --release -- \
		--osm=../data/input/$name.osm \
		--elevation=../data/input/N47W122.hgt \
		--traffic_signals=../data/input/TrafficSignals.shp \
		--parcels=../data/shapes/parcels \
		--gtfs=../data/input/google_transit_2018_18_08 \
		--neighborhoods=../data/input/neighborhoods.geojson \
		--output=../data/raw_maps/$name.abst
done
