# A/B Street + Mapbox demo

What is it? Goals?

## How it works

## How to run

You'll need `wasm-pack` and `npm` setup. You'll also need the `data/system/` directory to contain some maps.

Quick development: `wasm-pack build --dev --target web -- --features wasm && npm start`

To build the WASM in release mode: `wasm-pack build --release --target web -- --features wasm && npm start`

Maps can be specified by URL:
- http://localhost:8080/?map=/data/system/us/seattle/maps/arboretum.bin
