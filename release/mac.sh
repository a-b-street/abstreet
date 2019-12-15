#!/bin/bash
# Call from project root directory: ./release/mac.sh

set -e

version=$1;
if [ "$version" == "" ]; then
	echo Gimme version number
	exit 1
fi

OUT="abstreet_mac_$version"

dtrx abstreet_linux_$version.zip
mv abstreet_linux_$version $OUT
rm -fv $OUT/game/game
cp /media/dabreegster/PATRIOTUSB/game $OUT/game
chmod +x $OUT/game/game

zip -r $OUT $OUT
rm -rf $OUT
