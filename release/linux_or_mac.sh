#!/bin/bash
# Call from project root directory: ./release/linux.sh

set -e

version=$1;
if [ "$version" == "" ]; then
	echo Gimme version number
	exit 1
fi

OUT="abstreet_linux_$version"
if [[ "$OSTYPE" == "darwin"* ]]; then
	OUT="abstreet_mac_$version"
fi

source release/common.sh
common_release $OUT

cp release/play_abstreet.sh $OUT

mkdir $OUT/game
cargo build --release --bin game
cp target/release/game $OUT/game
cp -Rv game/assets $OUT/game

zip -r $OUT $OUT
rm -rf $OUT
