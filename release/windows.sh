#!/bin/bash
# Call from project root directory: ./release/windows.sh

set -e

OUT=abstreet_windows

source release/common.sh
common_release $OUT

cp release/play_abstreet.bat $OUT

mkdir $OUT/editor
cross build --release --target x86_64-pc-windows-gnu --bin editor
cp target/x86_64-pc-windows-gnu/release/editor.exe $OUT/editor

zip -r $OUT $OUT
rm -rf $OUT
