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
# Reuse what's in S3. But having a bunch of GCE VMs grab from S3 is expensive,
# so instead, sync from the GCS mirror that I manually update before each job.
gsutil -m cp -r gs://abstreet-importer/ .
mv abstreet-importer/dev/data/input data/input
rm -rf abstreet-importer
find data/input -name '*.gz' -print -exec gunzip '{}' ';'

# Install GDAL
sudo apt-get update
sudo apt-get install -y libgdal-dev

# Now do the big import!
rm -fv data/input/us/seattle/raw_maps/huge_seattle.bin data/input/us/seattle/popdat.bin
./target/release/cli regenerate-everything --shard-num=$WORKER_NUM --num-shards=$NUM_WORKERS

# Upload the results
./target/release/updater incremental-upload --version=$EXPERIMENT_TAG

# Indicate this VM is done by deleting ourselves. We can't use suspend or stop
# with a local SSD, so just nuke ourselves instead.
ZONE=$(curl -H Metadata-Flavor:Google http://metadata.google.internal/computeMetadata/v1/instance/zone -s | cut -d/ -f4)
echo y | gcloud compute instances delete $HOSTNAME --zone=$ZONE
