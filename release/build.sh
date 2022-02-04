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

for name in game cli fifteen_min osm_viewer parking_mapper santa ltn; do
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
