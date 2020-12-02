#!/bin/bash

set -e
wasm-pack build --dev --target web -- --no-default-features --features wasm
cd pkg
python3 -m http.server 8000
