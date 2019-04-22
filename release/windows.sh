#!/bin/bash
# Call from project root directory: ./release/windows.sh

set -e

OUT=abstreet_windows
rm -rfv $OUT
mkdir $OUT

cp color_scheme docs/INSTRUCTIONS.md release/play_abstreet.bat $OUT

mkdir -p $OUT/data/maps
cp -v data/maps/montlake.abst data/maps/23rd.abst $OUT/data/maps/

mkdir $OUT/editor
cross build --release --target x86_64-pc-windows-gnu --bin editor
cp target/x86_64-pc-windows-gnu/release/editor.exe $OUT/editor

zip -r $OUT $OUT
rm -rf $OUT
