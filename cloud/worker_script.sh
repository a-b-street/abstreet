#!/bin/bash
# This script runs inside of GCE VMs created by start_batch_import.sh. It
# imports a bunch of cities, then uploads the results to a temporary
# subdirectory in S3.

set -e
set -x

EXPERIMENT_TAG=$1
WORKER_NUM=$2
NUM_WORKERS=$3

if [ "$EXPERIMENT_TAG" == "" ] || [ "$WORKER_NUM" == "" ] || [ "$NUM_WORKERS" == "" ]; then
	echo Missing args;
	exit 1;
fi

# Install the AWS CLI
curl "https://awscli.amazonaws.com/awscli-exe-linux-x86_64.zip" -o "awscliv2.zip"
unzip awscliv2.zip
sudo ./aws/install

cd worker_payload
# Put the credentials in the right place
mv .aws ~/

# If we import without raw files, we'd wind up downloading fresh OSM data!
# Reuse what's in S3.

# OPTION 1 -- use the updater
#mkdir data/player
#./target/release/updater --opt-into-all-input > data/player/data.json
#./target/release/updater

# OPTION 2 -- probably aws has implemented fast file sync better than the
# updater. ;)
aws s3 sync s3://abstreet/dev/data/input data/input/
find data/input -name '*.gz' -print -exec gunzip '{}' ';'

# Now do the big import!
# TODO Should we rm -fv data/input/us/seattle/raw_maps/huge_seattle.bin
# data/input/us/seattle/raw_maps/huge_seattle.bin
# data/input/us/seattle/popdat.bin and regenerate? I think that'll require
# GDAL.
./target/release/importer --regen_all --shard_num=$WORKER_NUM --num_shards=$NUM_WORKERS

# Upload the results
./target/release/updater --inc_upload --version=$EXPERIMENT_TAG

# TODO Shutdown the VM, as an easy way of knowing when it's done?
