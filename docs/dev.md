# Developer guide

## Getting started

You will first need:

- Standard dependencies: `bash`, `curl`, `unzip`, `gunzip`
- Rust, at least 1.38. https://www.rust-lang.org/tools/install

One-time setup:

1.  Download the repository:
    `git clone https://github.com/dabreegster/abstreet.git`

2.  Build all input data. This is very slow, so you should seed from a pre-built
    copy: `./data/grab_seed_data.sh`. This will download about 1GB and expand to
    about 5GB.

3.  Run the game: `cd game; cargo run --release`

## Development tips

- Compile faster by just doing `cargo run`. The executable will have debug stack
  traces and run more slowly. You can do `cargo run --release` to build in
  optimized release mode; compilation will be slower, but the executable much
  faster.
- To add some extra debug modes to the game, `cargo run -- --dev` or press
  Control+S to toggle in-game
- All code is automatically formatted using
  https://github.com/rust-lang/rustfmt; please run `cargo fmt` before sending a
  PR.

## Building map data

You can skip this section if you're just touching code in `game`, `ezgui`, and
`sim`.

You'll need some extra dependencies:

- `osmconvert`: See https://wiki.openstreetmap.org/wiki/Osmconvert#Download
- `cs2cs` from proj4: See https://proj.org

The seed data from `data/grab_seed_data.sh` can be built from scratch by doing
`./import.sh && ./precompute.sh --release`. This takes a while.

Some tips:

- If you're modifying the initial OSM data -> RawMap conversion in
  `convert_osm`, then you do need to rerun `./import.sh` and `precompute.sh` to
  regenerate the map.
- If you're modifying `map_model` but not the OSM -> RawMap conversion, then you
  can just do `precompute.sh`.
- Both of those scripts can just regenerate a single map, which is much faster:
  `./import.sh caphill; ./precompute.sh caphill`
