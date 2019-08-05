#!/bin/bash
# Call from project root directory: ./release/linux.sh

set -e

OUT=abstreet_linux
if [[ "$OSTYPE" == "darwin"* ]]; then
	OUT=abstreet_mac
fi

source release/common.sh
common_release $OUT

cp release/play_abstreet.sh $OUT

mkdir $OUT/editor
cargo build --release --bin editor
cp target/release/editor $OUT/editor

zip -r $OUT $OUT
rm -rf $OUT
