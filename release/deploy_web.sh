#!/bin/bash

set -e
cd game
wasm-pack build --release --target web -- --no-default-features --features wasm,wasm_s3
cd pkg
aws s3 sync . s3://abstreet
echo "Have the appropriate amount of fun: http://abstreet.s3-website.us-east-2.amazonaws.com/"
