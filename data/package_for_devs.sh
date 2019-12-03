#!/bin/bash
# Run from the base repo directory: ./data/package_for_devs.sh

set -e

if [ "$USER" != "dabreegster" ]; then
	echo "Only Dustin runs this script, to help new developers avoid a long data import process.";
	exit 1;
fi

zip -r seed_data data/input data/system
echo "Fire at will: mv seed_data.zip ~/Dropbox"
