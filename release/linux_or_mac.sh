#!/bin/bash
# Call from project root directory: ./release/linux.sh

set -e

OUT=abstreet_linux
if [[ "$OSTYPE" == "darwin"* ]]; then
	OUT=abstreet_mac
fi

rm -rfv $OUT
mkdir $OUT

cp docs/INSTRUCTIONS.md release/play_abstreet.sh $OUT
mkdir $OUT/data
cp data/color_scheme.json $OUT/data

mkdir $OUT/data/maps
for map in 23rd ballard caphill downtown montlake; do
	cp -v data/maps/$map.bin $OUT/data/maps/
	mkdir -p $OUT/data/scenarios/$map
	cp -v data/scenarios/$map/psrc* $OUT/data/scenarios/$map/
done

mkdir $OUT/data/shapes
cp -v data/shapes/popdat.bin $OUT/data/shapes

mkdir $OUT/editor
cargo build --release --bin editor
cp target/release/editor $OUT/editor

zip -r $OUT $OUT
rm -rf $OUT
