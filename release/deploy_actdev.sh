#!/bin/bash
# This is like deploy_web.sh, but just creates a directory for https://github.com/cyipt/actdev

set -e

mkdir -p abst_actdev
cd game
#wasm-pack build --release --target web -- --no-default-features --features wasm,map_gui/wasm_s3
# Temporarily remove the symlink to the data directory
rm -f pkg/system
# Expand symlinks
cp -Hv pkg/* ../abst_actdev
# Restore the symlink
git checkout pkg/system
cd ..

# Copy just what's needed from data
mkdir abst_actdev/system
for dir in cambridge cheshire; do
	cp -Rv data/system/$dir abst_actdev/system
done
cp -Rv data/system/study_areas abst_actdev/system
