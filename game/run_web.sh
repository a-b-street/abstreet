#!/bin/bash

set -e
wasm-pack build --dev --target web -- --no-default-features --features wasm
cp index.html pkg
cd pkg
rm -f system
ln -s ../../data/system/ .
python3 -m http.server 8000
