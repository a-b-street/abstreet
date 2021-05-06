#!/bin/bash
# This script packages up the importer as it exists in the current git repo,
# deploys it to AWS Batch, and regenerates maps and scenarios for all cities.
#
# This process is only runnable by Dustin, due to current S3/EC2 permissions.
#
# Run from the repo's root dir: cloud/workflow.sh

set -e
set -x

EXPERIMENT_TAG=$1
if [ "$EXPERIMENT_TAG" == "" ]; then
	echo Missing args;
	exit 1;
fi

# It's a faster workflow to copy the local binaries into Docker, rather than
# build them inside the container. But it does require us to build the importer
# without the GDAL bindings, since the dynamic linking won't transfer over to
# the Docker image.
#
# GDAL bindings are only used when initially building popdat.bin for Seatle;
# there's almost never a need to regenerate this, and it can be done locally
# when required.
cargo build --release --bin importer --bin updater

docker build -f cloud/Dockerfile -t importer .
# To manually play around with the container: docker run -it importer /bin/bash

# TODO Upload the image to Docker Hub with a user-specified experiment tag
# TODO Kick off an AWS batch job
