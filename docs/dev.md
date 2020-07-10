# Developer guide

## Getting started

You will first need:

- Stable Rust, at least 1.43. https://www.rust-lang.org/tools/install.
  - On Windows, you may need
    [Visual Studio 2019](https://visualstudio.microsoft.com/de/downloads/).
- On Linux, `sudo apt-get install xorg-dev libxcb-shape0-dev libxcb-xfixes0-dev`
  or the equivalent for your distro

One-time setup:

1.  Download the repository:
    `git clone https://github.com/dabreegster/abstreet.git`

2.  Grab the minimal amount of data to get started: `cargo run --bin updater`

3.  Run the game: `RUST_BACKTRACE=1 cargo run --bin game --release`. On Windows,
    set environment variables like this:
    `set RUST_BACKTRACE=1 && cargo run --bin game --release`

## Development tips

- Compile faster by just doing `cargo run`. The executable will have debug stack
  traces and run more slowly. You can do `cargo run --release` to build in
  optimized release mode; compilation will be slower, but the executable much
  faster.
- Some in-game features are turned off by default or don't have a normal menu to
  access them. The list:
  - To toggle developer mode: press **Control+S** in game, or
    `cargo run -- --dev`
  - To warp to an object by numeric ID: press **Control+j**
  - To enter debug mode with all sorts of goodies: press **Control+D**
- You can start the game in different modes using flags:
  - `cargo run --bin game -- --dev data/system/maps/downtown.bin` starts on a
    particular map
  - `cargo run --bin game -- data/system/scenarios/downtown/weekday.bin` starts
    with a scenario (which is tied to a certain map)
  - `cargo run --bin game -- --challenge=trafficsig/tut2` starts on a particular
    challenge. See the list of aliases by passing in a bad value here.
  - `cargo run --bin game -- data/player/saves/montlake/no_edits_unnamed/00h00m20.3s.bin`
    restores an exact simulation state. Savestates are found in debug mode
    (**Control+D**) -- they're probably confusing for the normal player
    experience, so they're hidden for now.
  - `cargo run --bin game -- --tutorial=12` starts somewhere in the tutorial
  - Adding `--edits='name of edits'` starts with edits applied to the map.
- All code is automatically formatted using
  https://github.com/rust-lang/rustfmt; please run `cargo +nightly fmt` before
  sending a PR. (You have to install the nightly toolchain just for fmt)
- More random notes [here](/docs/misc_dev_tricks.md)

## Downloading more cities

As data formats change over time, things in the `data/` directory not under
version control will get out of date. At any time, you can run
`cargo run --bin updater` from the main repository directory to update only the
files that have changed.

You can also opt into downloading updates for more cities by editing
`data/config`. Opting into everything looks like this:

```
runtime: seattle,huge_seattle,austin,krakow
input: seattle,huge_seattle,austin,krakow
```

`runtime` downloads new maps and scenarios in `data/system/`. `input` is used
for building those maps -- see below.

## Building map data

You can skip this section if you're just touching code in `game`, `ezgui`, and
`sim`.

To run all pieces of the importer, you'll need some extra dependencies:

- `osmconvert`: See https://wiki.openstreetmap.org/wiki/Osmconvert#Download or
  https://github.com/interline-io/homebrew-planetutils#installation for Mac
- `libgdal-dev`: See https://gdal.org/ if your OS package manager doesn't have
  this. If you keep hitting linking errors, then just remove
  `--features scenarios` from `import.sh`. You won't be able to build the
  Seattle scenarios.
- Standard Unix utilities: `curl`, `unzip`, `gunzip`

The first stage of the importer, `--raw`, will download input files from OSM,
King County GIS, and so on. If the mirrors are slow or the files vanish, you
could fill out `data/config` and use the `updater` described above to grab the
latest input.

You can rerun specific stages of the importer:

- If you're modifying the initial OSM data -> RawMap conversion in
  `convert_osm`, you need `./import.sh --raw --map`.
- If you're modifying `map_model` but not the OSM -> RawMap conversion, then you
  just need `./import.sh --map`.
- By default, all maps are regenerated. You can also specify a single map:
  `./import.sh --map downtown`.
- By default, Seattle is assumed as the city. You have to specify otherwise:
  `./import.sh --city=los_angeles --map downtown_la`.

You can also make the importer [import a new city](new_city.md).

## Understanding stuff

The docs listed at https://github.com/dabreegster/abstreet#documentation explain
things like map importing and how the traffic simulation works.

### Code organization

If you're going to dig into the code, it helps to know what all the crates are.
The most interesting crates are `map_model`, `sim`, and `game`.

Constructing the map:

- `convert_osm`: extract useful data from OpenStreetMap and other data sources,
  emit intermediate map format
- `kml`: extract shapes from KML shapefiles
- `map_model`: the final representation of the map, also conversion from the
  intermediate map format into the final format
- `map_editor`: GUI for modifying geometry of maps and creating maps from
  scratch. pretty abandoned as of June 2020
- `importer`: tool to run the entire import pipeline
- `updater`: tool to download/upload large files used in the import pipeline

Traffic simulation:

- `sim`: all of the agent-based simulation logic
- `headless`: tool to run a simulation without any visualization

Graphics:

- `game`: the GUI and main gameplay
- `ezgui`: a GUI and 2D OpenGL rendering library, using glium + winit + glutin

Common utilities:

- `abstutil`: a grab-bag of IO helpers, timing and logging utilities, etc
- `geom`: types for GPS and map-space points, lines, angles, polylines,
  polygons, circles, durations, speeds
