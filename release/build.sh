#!/bin/bash
# Called by Github Actions workflow

set -e

output=$1;
runner=$2;
binary=$3;

./data/grab_minimal_seed_data.sh

mkdir $output

cp docs/INSTRUCTIONS.md $output
cp release/$runner $output
mkdir $output/game
cp $binary $output/game
cp -Rv data $output/data

# TODO Github will double-zip this, but if we just pass the directory, then the
# chmod +x bits get lost
zip -r $output $output
rm -rf release_data.zip $output
