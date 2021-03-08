#!/bin/bash
# This script imports scenarios an actdev site that's already been imported.
# It's run manually once per site.

set -e

CITY=$1
if [ "$CITY" == "" ]; then
	echo Missing args;
	exit 1;
fi
SITE=`echo $CITY | sed -r 's/_/-/g'`

rm -fv *.json
wget https://raw.githubusercontent.com/cyipt/actdev/main/data-small/$SITE/scenario_base.json
wget https://raw.githubusercontent.com/cyipt/actdev/main/data-small/$SITE/scenario_go_active.json

cargo run --release --bin import_traffic -- --map=data/system/gb/$CITY/maps/center.bin --input=scenario_base.json --skip_problems
cargo run --release --bin import_traffic -- --map=data/system/gb/$CITY/maps/center.bin --input=scenario_go_active.json --skip_problems
rm -fv *.json
./import.sh --scenario --city=gb/$CITY
