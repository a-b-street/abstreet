#!/bin/bash
# This assumes you previously ran bbike.sh and tries to extract and import all
# cities from there. You'll also need https://www.gnu.org/software/parallel/.
#
# Be warned, running this eats CPU and disk space.

set -e

# Dump lots of temporary output here
mkdir -p mass_import
cd mass_import

# First extract all "cities" from the huge bbike files. If two names collide,
# the .osm and .poly might mix between the two arbitrarily!
# Don't parallelize (-j1); I think osmconvert must eat CPUs, because my system
# lags heavily with -j4 here.
for raw_extract in `ls ~/bbike_extracts`; do
	raw_extract=`basename -s .osm $raw_extract`
	echo "cargo run --release --bin extract_cities -- /home/$USER/bbike_extracts/$raw_extract.osm --radius_around_label_miles=6 > extract_$raw_extract.log 2>&1"
done | parallel --bar -j1

# Spaces in filenames will mess stuff up
# If no files have spaces, the loop fails, so temporarily set +e
set +e
for f in *\ *; do
	mv "$f" "${f// /_}"
done
set -e

# Then import each smaller .osm
cd ..
for name in `ls mass_import/*.osm`; do
	name=`basename -s .osm $name`
	echo "./import.sh --oneshot=mass_import/$name.osm --oneshot_clip=mass_import/$name.poly --skip_ch > mass_import/import_$name.log 2>&1"
done | parallel --bar -j4
