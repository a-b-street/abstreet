#!/bin/bash
# Call from project root directory: ./release/linux.sh

set -e

OUT=abstreet_linux
rm -rfv $OUT
mkdir $OUT

cp color_scheme docs/INSTRUCTIONS.md release/play_abstreet.sh $OUT

mkdir -p $OUT/data/maps
cp -v data/maps/montlake.abst data/maps/23rd.abst $OUT/data/maps/

mkdir $OUT/editor
cargo build --release --bin editor
cp target/release/editor $OUT/editor

zip -r $OUT $OUT
rm -rf $OUT
