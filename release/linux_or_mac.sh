#!/bin/bash
# Call from project root directory: ./release/linux.sh

set -e

OUT=abstreet_linux
if [[ "$OSTYPE" == "darwin"* ]]; then
	OUT=abstreet_mac
fi

rm -rfv $OUT
mkdir $OUT

cp color_scheme docs/INSTRUCTIONS.md release/play_abstreet.sh $OUT

mkdir -p $OUT/data/maps
for map in 23rd ballard caphill downtown montlake; do
	cp -v data/maps/$map.abst $OUT/data/maps/
done

mkdir -p $OUT/data/shapes
cp -v data/shapes/popdat $OUT/data/shapes

mkdir $OUT/editor
cargo build --release --bin editor
cp target/release/editor $OUT/editor

zip -r $OUT $OUT
rm -rf $OUT
