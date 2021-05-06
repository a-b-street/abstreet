#!/bin/bash
# This script runs inside the abst importer Docker container. It imports a
# single city, then pushes the results to a temporary subdirectory in S3.

set -e
set -x

EXPERIMENT_TAG=$1
CITY=$2
if [ "$EXPERIMENT_TAG" == "" ] || [ "$CITY" == "" ]; then
	echo Missing args;
	exit 1;
fi

# If we import --raw without any files, we would wind up downloading fresh OSM
# data. We want to reuse whatever's in S3, and explicitly grab fresh OSM
# through a different process.
mkdir -p data/player
echo "{\"runtime\": [], \"input\": [\"$CITY\"]}" > data/player/data.json
./target/release/updater

# TODO --scenario for some cities
./target/release/importer --raw --map --city=$CITY
