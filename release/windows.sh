#!/bin/bash
# Call from project root directory: ./release/windows.sh

set -e

OUT=abstreet_windows
rm -rfv $OUT
mkdir $OUT

cp color_scheme INSTRUCTIONS.md $OUT
mkdir -p $OUT/data/maps
for map in montlake 23rd; do
	cp -v data/maps/$map.abst $OUT/data/maps/
	cat << EOF > $OUT/run_$map.bat
cd editor
editor.exe ..\\data\\maps\\$map.abst > ..\\output.txt
EOF
done

mkdir $OUT/editor
cross build --release --target x86_64-pc-windows-gnu --bin editor
cp target/x86_64-pc-windows-gnu/release/editor.exe $OUT/editor

zip -r $OUT $OUT
rm -rf $OUT
