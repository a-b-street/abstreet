#!/bin/bash
# Run from the base repo directory: ./data/package_for_releases.sh

set -e

if [ "$USER" != "dabreegster" ]; then
	echo "Only Dustin runs this script, to automate releases.";
	exit 1;
fi

mkdir release_data
cp -Rv data/system release_data
# Not worth blowing up the download size yet
rm -rfv release_data/system/maps/huge_seattle.bin release_data/system/scenarios/huge_seattle
rm -rfv release_data/system/scenarios/montlake/everyone_weekday.bin

zip -r release_data release_data
rm -rf release_data
mv -fv release_data.zip ~/Dropbox
