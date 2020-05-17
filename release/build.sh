#!/bin/bash
# Called by Github Actions workflow

set -e

output=$1;
runner=$2;
game_binary=$3;
importer_binary=$4;

cargo run --bin updater

mkdir $output

cp docs/INSTRUCTIONS.md $output
cp release/$runner $output
mkdir $output/game
cp $game_binary $output/game
mkdir $output/importer
cp $importer_binary $output/importer
mkdir $output/data
cp -Rv data/system $output/data/system

# TODO Github will double-zip this, but if we just pass the directory, then the
# chmod +x bits get lost
zip -r $output $output
rm -rf release_data.zip $output
