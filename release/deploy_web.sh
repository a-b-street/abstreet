#!/bin/bash

set -x
VERSION=dev
# S3_ROOT=s3://mjk_asdf/abstreet
S3_ROOT=s3://abstreet

set -e

cd web;
make release
aws s3 sync build/dist/ $S3_ROOT/$VERSION/

# Set the content type for .wasm files, to speed up how browsers load them
aws s3 cp \
       $S3_ROOT/$VERSION \
       $S3_ROOT/$VERSION \
       --exclude '*' \
       --include '*.wasm' \
       --no-guess-mime-type \
       --content-type="application/wasm" \
       --metadata-directive="REPLACE" \
       --recursive

echo "Have the appropriate amount of fun: http://abstreet.s3-website.us-east-2.amazonaws.com/$VERSION"
