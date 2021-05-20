#!/bin/bash
# This script grabs all files changed by VMs, copying them locally.
#
# This process is only runnable by Dustin, due to current S3 permissions.
#
# Run from the repo's root dir: cloud/gather_output.sh

set -e
set -x

EXPERIMENT_TAG=$1
if [ "$EXPERIMENT_TAG" == "" ]; then
	echo Missing args;
	exit 1;
fi

aws s3 cp --recursive s3://abstreet/$EXPERIMENT_TAG/data/ data/
# gunzip all of the changed files, overwriting the local copies
find data/ -path data/system/assets -prune -o -name '*.gz' -print -exec gunzip -f '{}' ';'

echo "Done! Validate the files, run updater --upload as usual, and don't forget to clean up s3://abstreet/$EXPERIMENT_TAG"
