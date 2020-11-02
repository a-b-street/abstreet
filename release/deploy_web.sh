#!/bin/bash

set -e
cd game
wasm-pack build --release --target web -- --no-default-features --features wasm,wasm_s3
cd pkg
# Temporarily point to the Dropbox data, which is gzipped
rm -f system
ln -s ~/Dropbox/abstreet_data/data/system/ system
aws s3 sync --delete --exclude '*/osm_demo/*' . s3://abstreet
# Undo that symlink swap
git checkout system
# Set the content type for .wasm files, to speed up how browsers load them
aws s3 cp \
       s3://abstreet/ \
       s3://abstreet/ \
       --exclude '*' \
       --include '*.wasm' \
       --no-guess-mime-type \
       --content-type="application/wasm" \
       --metadata-directive="REPLACE" \
       --recursive
echo "Have the appropriate amount of fun: http://abstreet.s3-website.us-east-2.amazonaws.com/"
