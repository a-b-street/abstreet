#!/bin/bash
# Called by Github Actions workflow

set -e;

os=$1;
case $os in
	ubuntu-18.04)
		output="abst_linux";
		suffix="";
		ext="sh";
		;;

	macos-latest)
		output="abst_mac";
		suffix="";
		ext="sh";
		;;

	windows-latest)
		output="abst_windows";
		suffix=".exe";
		ext="bat";
		;;

	*)
		echo "Wat? os = $os";
		exit 1;
esac

mkdir $output

cp release/play_abstreet.$ext release/ungap_the_map.$ext release/INSTRUCTIONS.txt $output

# Put most binaries in the root directory, but hide game to encourage people to
# use the launch scripts. They will capture output logs. But if somebody runs
# the game binary directly, it'll still work.
mkdir $output/game
cp target/release/game${suffix} $output/game
cp target/release/cli ${suffix} $output/cli

for name in fifteen_min osm_viewer parking_mapper santa; do
	cp target/release/${name}${suffix} $output;
done

mkdir $output/data
cp -Rv data/system $output/data/system
cp data/MANIFEST.json $output/data

# Windows doesn't have zip?!
if [[ "$output" != "abst_windows" ]]; then
	# TODO Github will double-zip this, but if we just pass the directory, then the
	# chmod +x bits get lost
	zip -r $output $output
	rm -rf release_data.zip $output
fi
