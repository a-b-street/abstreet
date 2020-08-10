#!/bin/bash
# Called by Github Actions workflow

set -e

output=$1;
runner=$2;
game_binary=$3;
importer_binary=$4;

cargo run --bin updater

mkdir $output

cp old_docs/INSTRUCTIONS.md $output
cp release/$runner $output
# Put the importer in the root directory, but hide game to encourage people to
# use the runner script. It will capture output logs. But if somebody runs the
# game binary directly, it'll still work.
mkdir $output/game
cp $game_binary $output/game
cp $importer_binary $output
mkdir $output/data
cp -Rv data/system $output/data/system

# Windows doesn't have zip?!
if [[ "$output" != "abst_windows" ]]; then
	# TODO Github will double-zip this, but if we just pass the directory, then the
	# chmod +x bits get lost
	zip -r $output $output
	rm -rf release_data.zip $output
fi
