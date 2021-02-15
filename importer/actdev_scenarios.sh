#!/bin/bash
# This script imports scenarios an actdev site that's already been imported.
# It's run manually once per site.

set -e

SITE=$1
if [ "$SITE" == "" ]; then
	echo Missing args;
	exit 1;
fi
CITY=`echo $SITE | sed -r 's/-/_/g'`

rm -fv *.json
wget https://raw.githubusercontent.com/cyipt/actdev/main/data-small/$SITE/scenario-base.json
wget https://raw.githubusercontent.com/cyipt/actdev/main/data-small/$SITE/scenario-godutch.json

cargo run --release --bin import_traffic -- --map=data/system/gb/$CITY/maps/center.bin --input=scenario-base.json --skip_problems
cargo run --release --bin import_traffic -- --map=data/system/gb/$CITY/maps/center.bin --input=scenario-godutch.json --skip_problems
rm -fv *.json
