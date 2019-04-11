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

if [ ! -f data/input/traffic_signals.kml ]; then
	# From https://data.seattle.gov/Transportation/Traffic-Signals/nr6x-wnd5
	get_if_needed \
		http://data-seattlecitygis.opendata.arcgis.com/datasets/ff97a6eb8ac84356beea09138c6e1ec3_0.kml \
		data/input/traffic_signals.kml;
fi

if [ ! -f data/input/neighborhoods.geojson ]; then
	# https://data.seattle.gov/dataset/Neighborhoods/2mbt-aqqx in GeoJSON, not SHP
	get_if_needed \
		https://github.com/seattleio/seattle-boundaries-data/raw/master/data/neighborhoods.geojson \
		data/input/neighborhoods.geojson;
fi

if [ ! -f data/input/residential_buildings.kml ]; then
	# From https://data-seattlecitygis.opendata.arcgis.com/datasets/residential-building-permits-issued-and-final
	get_if_needed \
		https://opendata.arcgis.com/datasets/cb8c492055a44f2f9de427e0518f9246_0.kml \
		data/input/residential_buildings.kml;
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

if [ ! -f data/shapes/blockface ]; then
	# From http://data-seattlecitygis.opendata.arcgis.com/datasets/blockface
	get_if_needed https://opendata.arcgis.com/datasets/a1458ad1abca41869b81f7c0db0cd777_0.kml data/input/blockface.kml;

	cd kml
	time cargo run --release -- \
		--input=../data/input/blockface.kml \
		--output=../data/shapes/blockface
	cd ..
fi

cd convert_osm
for poly in `ls ../data/polygons/`; do
	name=`basename -s .poly $poly`;
	rm -rf ../data/neighborhoods/$name ../data/maps/${name}_*.abst;
	RUST_BACKTRACE=1 cargo run --release -- \
		--osm=../data/input/$name.osm \
		--elevation=../data/input/N47W122.hgt \
		--traffic_signals=../data/input/traffic_signals.kml \
		--residential_buildings=../data/input/residential_buildings.kml \
		--parking_shapes=../data/shapes/blockface \
		--gtfs=../data/input/google_transit_2018_18_08 \
		--neighborhoods=../data/input/neighborhoods.geojson \
		--clip=../data/polygons/$name.poly \
		--output=../data/raw_maps/$name.abst
done

# To run manually: cargo run -- --osm=../data/input/montlake.osm --elevation=../data/input/N47W122.hgt --traffic_signals=../data/input/traffic_signals.kml --residential_buildings=../data/input/residential_buildings.kml --parking_shapes=../data/shapes/blockface --gtfs=../data/input/google_transit_2018_18_08 --neighborhoods=../data/input/neighborhoods.geojson --clip=../data/polygons/montlake.poly --output=../data/raw_maps/montlake.abst --fast_dev
