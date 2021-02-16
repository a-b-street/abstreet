#!/bin/bash
# This is like deploy_web.sh, but just creates a directory for https://github.com/cyipt/actdev

set -e

mkdir -p abst_actdev
cd game
wasm-pack build --release --target web -- --no-default-features --features wasm
# Temporarily remove the symlink to the data directory
rm -f pkg/system
# Expand symlinks
cp -Hv pkg/* ../abst_actdev
# Restore the symlink
git checkout pkg/system
cd ..

# Copy just what's needed from data
mkdir -p abst_actdev/system/gb
# We could just copy all of system/gb and remove leeds and london, but
# actually, having this list in a script somewhere is kind of convenient.
for dir in allerton_bywater ashton_park aylesbury aylesham bailrigg bath_riverside bicester castlemead chapelford clackers_brook culm dickens_heath didcot dunton_hills ebbsfleet great_kneighton hampton handforth kidbrooke_village lcid long_marston micklefield newcastle_great_park poundbury priors_hall taunton_firepool taunton_garden tresham trumpington_meadows tyersal_lane upton wichelstowe wixams; do
	cp -Rv data/system/gb/$dir abst_actdev/system/gb
done
cp -Rv data/system/study_areas abst_actdev/system
gzip `find abst_actdev/ | grep bin | xargs`

zip -r abst_actdev abst_actdev
rm -rf abst_actdev
echo "Go upload abst_actdev.zip to https://github.com/cyipt/actdev/releases"
