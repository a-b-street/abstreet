# A/B Street + Mapbox demo

This is an example of integrating parts of A/B Street with Mapbox GL. It's a
normal web app using Mapbox, but it includes a layer rendering streets and
moving agents from A/B Street.

The goal is to increase interoperability and meet developers where they're at.
Parts of the A/B Street code-base are intended as
[a platform](https://a-b-street.github.io/docs/tech/map/platform.html) to build
other transportation-related things, using unique features like the detailed
street rendering. But Rust and our unusual UI library are a huge barrier.
Treating A/B Street as a layer that can be added to Mapbox and as a library with
a simple API for controlling a traffic simulation should be an easier start.

Another goal is to take advantage of all the great stuff that exists in the web
ecosystem. Instead of implementing satellite layers, multi-line text entry
(seriously!), and story mapping ourselves, we can just use stuff that's built
already.

## How to run

You'll need `wasm-pack` and `python3` setup. You'll also need the `data/system/`
directory to contain some maps.

Quick development:
`wasm-pack build --dev --target web -- --features wasm && ./serve_locally.py`

To build the WASM in release mode:
`wasm-pack build --release --target web -- --features wasm && ./serve_locally.py`

Maps can be specified by URL:

- http://localhost:8000/?map=/data/system/us/seattle/maps/arboretum.bin

No deployment instructions yet.

## How it works

The `PiggybackDemo` struct is a thin layer written in Rust to hook up to the
rest of the A/B Street codebase. After being initialized with a WebGL context and
a map file, it can render streets and agents and control a traffic simulation.
It serves as a public API, exposed via WASM. Then a regular Mapbox GL app treats
it as a library and adds a custom WebGL rendering layer that calls this API.
