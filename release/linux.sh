#!/bin/bash
# Call from project root directory: ./release/linux.sh

set -e

OUT=abstreet_linux
rm -rfv $OUT
mkdir $OUT

cp color_scheme INSTRUCTIONS.md $OUT
mkdir -p $OUT/data/maps
for map in montlake 23rd; do
	cp -v data/maps/$map.abst $OUT/data/maps/
done

mkdir $OUT/editor
cargo build --release --bin editor
cp target/release/editor $OUT/editor

zip -r $OUT $OUT
rm -rf $OUT
