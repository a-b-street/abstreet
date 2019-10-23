#!/bin/bash

set -e

only_map=""
release="--release"
for arg in "$@"; do
	if [ "$arg" == "--debug" ]; then
		release="";
	else
		only_map=$arg;
	fi
done

# First prepare input.

function get_if_needed {
	if [ ! -f $2 ]; then
		wget $1 -O $2;
	fi
}

mkdir -p data/input data/raw_maps

# TODO refactor a variant for .zips?
if [ ! -d data/input/google_transit_2018_18_08/ ]; then
	get_if_needed \
		https://metro.kingcounty.gov/GTFS/google_transit_2018_18_08.zip \
		data/input/google_transit_2018_18_08.zip;
	unzip -d data/input/google_transit_2018_18_08 data/input/google_transit_2018_18_08.zip;
	rm -f data/input/google_transit_2018_18_08.zip;
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

# PSRC data comes from https://github.com/psrc/soundcast/releases.
if [ ! -f data/input/parcels_urbansim.txt ]; then
	get_if_needed https://www.dropbox.com/s/t9oug9lwhdwfc04/psrc_2014.zip?dl=0 data/input/psrc_2014.zip;
	unzip data/input/psrc_2014.zip -d data/input;
	rm -f data/input/psrc_2014.zip;
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

if [ ! -f data/shapes/blockface.bin ]; then
	# From http://data-seattlecitygis.opendata.arcgis.com/datasets/blockface
	get_if_needed https://opendata.arcgis.com/datasets/a1458ad1abca41869b81f7c0db0cd777_0.kml data/input/blockface.kml;

	cd kml
	time cargo run --release -- \
		--input=../data/input/blockface.kml \
		--output=../data/shapes/blockface.bin
	cd ..
fi

if [ ! -f data/shapes/sidewalks.bin ]; then
	# From https://data-seattlecitygis.opendata.arcgis.com/datasets/sidewalks
	get_if_needed https://opendata.arcgis.com/datasets/ee6d0642d2a04e35892d0eab77d971d6_2.kml data/input/sidewalks.kml;

	cd kml
	time cargo run --release -- \
		--input=../data/input/sidewalks.kml \
		--output=../data/shapes/sidewalks.bin
	cd ..
fi

if [ ! -f data/input/household_vehicles.kml ]; then
	# From https://gis-kingcounty.opendata.arcgis.com/datasets/acs-household-size-by-vehicles-available-acs-b08201-householdvehicles
	get_if_needed https://opendata.arcgis.com/datasets/7842d815523c4f1b9564e0301e2eafa4_2372.kml data/input/household_vehicles.kml;
	get_if_needed https://www.arcgis.com/sharing/rest/content/items/7842d815523c4f1b9564e0301e2eafa4/info/metadata/metadata.xml data/input/household_vehicles.xml;
fi

if [ ! -f data/input/commute_time.kml ]; then
	# From https://gis-kingcounty.opendata.arcgis.com/datasets/acs-travel-time-to-work-acs-b08303-traveltime
	get_if_needed https://opendata.arcgis.com/datasets/9b5fd85861a04c5ab8b7407c7b58da7c_2375.kml data/input/commute_time.kml;
	get_if_needed https://www.arcgis.com/sharing/rest/content/items/9b5fd85861a04c5ab8b7407c7b58da7c/info/metadata/metadata.xml data/input/commute_time.xml;
fi

if [ ! -f data/input/commute_mode.kml ]; then
	# From https://gis-kingcounty.opendata.arcgis.com/datasets/acs-means-of-transportation-to-work-acs-b08301-transportation
	get_if_needed https://opendata.arcgis.com/datasets/1da9717ca5ff4505826aba40a7ac0a58_2374.kml data/input/commute_mode.kml;
	get_if_needed https://www.arcgis.com/sharing/rest/content/items/1da9717ca5ff4505826aba40a7ac0a58/info/metadata/metadata.xml data/input/commute_mode.xml;
fi

if [ ! -f data/input/offstreet_parking.kml ]; then
	# From https://data.seattle.gov/Transportation/Public-Garages-or-Parking-Lots/xefx-khzm
	get_if_needed http://data-seattlecitygis.opendata.arcgis.com/datasets/8e52dfde6d5d45948f7a90654c8d50cd_0.kml data/input/offstreet_parking.kml;
fi

cd convert_osm
for poly in `ls ../data/polygons/`; do
	name=`basename -s .poly $poly`;
	if [ "$only_map" != "" ] && [ "$only_map" != "$name" ]; then
		continue;
	fi

	rm -rf ../data/neighborhoods/$name ../data/maps/${name}.bin;
	RUST_BACKTRACE=1 cargo run $release -- \
		--osm=../data/input/$name.osm \
		--parking_shapes=../data/shapes/blockface.bin \
		--offstreet_parking=../data/input/offstreet_parking.kml \
		--gtfs=../data/input/google_transit_2018_18_08 \
		--neighborhoods=../data/input/neighborhoods.geojson \
		--clip=../data/polygons/$name.poly \
		--output=../data/raw_maps/$name.bin
done
