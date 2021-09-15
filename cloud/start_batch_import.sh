#!/bin/bash
# This script packages up the importer as it exists in the current git repo,
# creates a bunch of GCE VMs, and runs the importer there on all cities, using
# static sharding.
#
# This process is only runnable by Dustin, due to current GCE/S3 permissions.
#
# Run from the repo's root dir: cloud/start_batch_import.sh

set -e
set -x

EXPERIMENT_TAG=$1
if [ "$EXPERIMENT_TAG" == "" ]; then
	echo Missing args;
	exit 1;
fi

if [ "$2" != "gcs_sync_done" ]; then
	echo First go sync dev/data/input from S3 to GCS. https://console.cloud.google.com/transfer/cloud/jobs
	exit 1;
fi

NUM_WORKERS=10
ZONE=us-east1-b
# See other options: https://cloud.google.com/compute/docs/machine-types
# Particularly... e2-standard-2, n2-standard-2, c2-standard-4
SMALL_MACHINE_TYPE=e2-standard-2
LARGE_MACHINE_TYPE=c2-standard-4
# All of data/ is currently around 30GB
DISK_SIZE=40GB
# Compressing and checksumming gigantic files needs more IOPS
# TODO But wait, e2-standard-2 doesn't support local PD?!
DISK_TYPE=pd-ssd
# Haha, using a project from college, my last traffic sim...
PROJECT=aorta-routes

function build_payload {
	# It's a faster workflow to copy the local binaries into the VMs, rather than
	# build them there.
	cargo build --release --bin updater
	# This is needed to regenerate popdat.bin for Seattle. It requires the
	# VMs to have a matching GDAL version. Since the VM runs the same
	# version of Ubuntu as I do locally, this works out fine.
	cargo build --release --bin cli --features importer/scenarios

	# Build our payload for the VMs
	# This mkdir deliberately fails if the directory is already there; it probably
	# means the last run broke somehow
	mkdir worker_payload
	mkdir -p worker_payload/target/release
	cp target/release/cli worker_payload/target/release/cli
	cp target/release/updater worker_payload/target/release/
	mkdir worker_payload/data
	cp data/MANIFEST.json worker_payload/data
	mkdir worker_payload/importer
	cp -Rv importer/config worker_payload/importer
	cp cloud/worker_script.sh worker_payload/
	# Copy in AWS credentials! Obviously don't go making worker_payload/ public or
	# letting anybody into the VMs.
	#
	# Alternatively, I could just scp the files from the VMs back to my local
	# computer. But more than likely, GCE's upstream speed to S3 (even
	# cross-region) is better than Comcast. :)
	cp -Rv ~/.aws worker_payload/
	zip -r worker_payload worker_payload
}

function create_vms {
	# Ideally we'd use the bulk API, but someone's not on top of those
	# gcloud integration tests...
	# https://issuetracker.google.com/issues/188462253
	for ((i = 0; i < $NUM_WORKERS; i++)); do
		# The first shard always handles Seattle, which needs more than
		# 8GB of memory. Just give it really hefty hardware.
		if [ $i == 0 ]; then
			machine_type=$LARGE_MACHINE_TYPE;
		else
			machine_type=$SMALL_MACHINE_TYPE;
		fi

		gcloud compute \
			--project=$PROJECT \
			instances create "worker-$i" \
			--zone=$ZONE \
			--machine-type=$machine_type \
			--boot-disk-size=$DISK_SIZE \
			--boot-disk-type=$DISK_TYPE \
			--image-family=ubuntu-2004-lts \
			--image-project=ubuntu-os-cloud \
			--scopes=compute-rw,storage-ro
	done

	# There's a funny history behind the whole "how do I wait for my VM to be
	# SSHable?" question...
	sleep 30s
}

function start_workers {
	for ((i = 0; i < $NUM_WORKERS; i++)); do
		gcloud compute scp \
			--project=$PROJECT \
			--zone=$ZONE \
			worker_payload.zip \
			worker-$i:~/worker_payload.zip
		gcloud compute ssh \
			--project=$PROJECT \
			--zone=$ZONE \
			worker-$i \
			--command="sudo apt-get -qq install -y unzip; unzip -q worker_payload.zip; ./worker_payload/worker_script.sh $EXPERIMENT_TAG $i $NUM_WORKERS 1> logs 2>&1 &"
	done
}

build_payload
create_vms
start_workers

# To follow along with a worker:
# > gcloud compute ssh worker-5 --command='tail -f logs'
#
# To see which workers are still running (or have failed):
# > gcloud compute instances list
