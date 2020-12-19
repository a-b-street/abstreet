#!/bin/bash

VERSION=dev

set -e

# The parking mapper doesn't work on WASM yet, so don't include it
for tool in game santa fifteen_min osm_viewer; do
	cd $tool
	wasm-pack build --release --target web -- --no-default-features --features wasm,map_gui/wasm_s3
	cd pkg
	# Temporarily remove the symlink to the data directory; it's uploaded separately by the updater tool
	rm -f system
	aws s3 sync . s3://abstreet/$VERSION/$tool
	# Undo that symlink hiding
	git checkout system
	cd ../..
done

# Set the content type for .wasm files, to speed up how browsers load them
aws s3 cp \
       s3://abstreet/$VERSION \
       s3://abstreet/$VERSION \
       --exclude '*' \
       --include '*.wasm' \
       --no-guess-mime-type \
       --content-type="application/wasm" \
       --metadata-directive="REPLACE" \
       --recursive
echo "Have the appropriate amount of fun: http://abstreet.s3-website.us-east-2.amazonaws.com/$VERSION"
