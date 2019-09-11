#!/bin/bash
# Call from project root directory: ./release/windows.sh

set -e

OUT=abstreet_windows

source release/common.sh
common_release $OUT

cp release/play_abstreet.bat $OUT

mkdir $OUT/game
cross build --release --target x86_64-pc-windows-gnu --bin game
cp target/x86_64-pc-windows-gnu/release/game.exe $OUT/game
cp -Rv game/assets $OUT/game

zip -r $OUT $OUT
rm -rf $OUT
