#!/bin/bash
# This creates a .zip with all of the files needed to serve a copy of the Mapbox demo.

set -x
set -e

wasm-pack build --release --target web -- --features wasm

mkdir mapbox_demo
cp -Rv index.html serve_locally.py pkg mapbox_demo

mkdir -p mapbox_demo/data/system/us/seattle/maps
mkdir -p mapbox_demo/data/system/de/berlin/maps
# Just include a few maps
cp ../data/system/us/seattle/maps/montlake.bin mapbox_demo/data/system/us/seattle/maps
cp ../data/system/de/berlin/maps/neukolln.bin mapbox_demo/data/system/de/berlin/maps

# Uncomment with caution!
# Note this embeds a tiny slice of the data/ directory underneath mapbox_demo.
# The S3 bucket has gzipped map files, but the JS / Rust layers don't handle
# reading both yet.
#aws s3 sync mapbox_demo s3://abstreet/dev/mapbox_demo
