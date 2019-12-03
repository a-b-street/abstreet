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
		echo "Downloading $1";
		curl -o $2 $1;
	fi
}

mkdir -p data/input/raw_maps

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

for poly in `ls data/input/polygons/`; do
	name=`basename -s .poly $poly`;
	if [ ! -f data/input/$name.osm ]; then
		echo "Running osmconvert for $name"
		osmconvert data/input/Seattle.osm \
			-B=data/input/polygons/$name.poly \
			--complete-ways \
			-o=data/input/$name.osm
	fi
done

if [ ! -f data/input/blockface.bin ]; then
	# From http://data-seattlecitygis.opendata.arcgis.com/datasets/blockface
	get_if_needed https://opendata.arcgis.com/datasets/a1458ad1abca41869b81f7c0db0cd777_0.kml data/input/blockface.kml;

	cd kml
	time cargo run --release -- \
		--input=../data/input/blockface.kml \
		--output=../data/input/blockface.bin
	rm -f data/input/blockface.kml;
	cd ..
fi

if [ ! -f data/input/sidewalks.bin ]; then
	# From https://data-seattlecitygis.opendata.arcgis.com/datasets/sidewalks
	get_if_needed https://opendata.arcgis.com/datasets/ee6d0642d2a04e35892d0eab77d971d6_2.kml data/input/sidewalks.kml;

	cd kml
	time cargo run --release -- \
		--input=../data/input/sidewalks.kml \
		--output=../data/input/sidewalks.bin
	rm -f data/input/sidewalks.kml;
	cd ..
fi

if [ ! -f data/input/offstreet_parking.kml ]; then
	# From https://data.seattle.gov/Transportation/Public-Garages-or-Parking-Lots/xefx-khzm
	get_if_needed http://data-seattlecitygis.opendata.arcgis.com/datasets/8e52dfde6d5d45948f7a90654c8d50cd_0.kml data/input/offstreet_parking.kml;
fi

cd convert_osm
for poly in `ls ../data/input/polygons/`; do
	name=`basename -s .poly $poly`;
	if [ "$only_map" != "" ] && [ "$only_map" != "$name" ]; then
		continue;
	fi

	rm -rf ../data/input/neighborhoods/$name ../data/system/maps/${name}.bin;
	RUST_BACKTRACE=1 cargo run $release -- \
		--osm=../data/input/$name.osm \
		--parking_shapes=../data/input/blockface.bin \
		--offstreet_parking=../data/input/offstreet_parking.kml \
		--gtfs=../data/input/google_transit_2018_18_08 \
		--neighborhoods=../data/input/neighborhoods.geojson \
		--clip=../data/input/polygons/$name.poly \
		--output=../data/input/raw_maps/$name.bin
		#--sidewalks=../data/input/sidewalks.bin \
done
