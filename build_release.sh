#!/bin/bash

set -e

OUT=abst_release

rm -rfv $OUT

mkdir $OUT
cp color_scheme README.md $OUT

mkdir $OUT/editor
#cp target/debug/editor $OUT/editor
#cp target/x86_64-pc-windows-gnu/debug/editor.exe $OUT/editor
cp target/x86_64-pc-windows-gnu/release/editor.exe $OUT/editor

mkdir -p $OUT/data/maps
cp data/maps/montlake_no_edits.abst $OUT/data/maps

zip -r $OUT $OUT
