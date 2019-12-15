#!/bin/bash
# Call from project root directory: ./release/linux.sh

set -e

version=$1;
if [ "$version" == "" ]; then
	echo Gimme version number
	exit 1
fi

OUT="abstreet_linux_$version"

source release/common.sh
common_release $OUT

cp release/play_abstreet.sh $OUT

mkdir $OUT/game
cross build --release --target x86_64-unknown-linux-gnu --bin game
cp target/x86_64-unknown-linux-gnu/release/game $OUT/game
cp -Rv game/assets $OUT/game

zip -r $OUT $OUT
rm -rf $OUT
