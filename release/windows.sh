#!/bin/bash
# Call from project root directory: ./release/windows.sh

set -e

OUT=abstreet_windows
rm -rfv $OUT
mkdir $OUT

cp color_scheme.json docs/INSTRUCTIONS.md release/play_abstreet.bat $OUT

mkdir -p $OUT/data/maps
for map in 23rd ballard caphill downtown montlake; do
	cp -v data/maps/$map.bin $OUT/data/maps/
done

mkdir -p $OUT/data/shapes
cp -v data/shapes/popdat.bin $OUT/data/shapes

mkdir $OUT/editor
cross build --release --target x86_64-pc-windows-gnu --bin editor
cp target/x86_64-pc-windows-gnu/release/editor.exe $OUT/editor

zip -r $OUT $OUT
rm -rf $OUT
