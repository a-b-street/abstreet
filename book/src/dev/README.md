# Developer guide

## Getting started

You will first need:

- Stable Rust, at least 1.47. <https://www.rust-lang.org/tools/install>.
  - On Windows, you may need
    [Visual Studio 2019](https://visualstudio.microsoft.com/en/downloads/).
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

- [Generated API documentation](https://dabreegster.github.io/abstreet/rustdoc/map_model/index.html)
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
  - `cargo run --bin game -- --dev data/system/seattle/maps/downtown.bin` starts
    on a particular map
  - `cargo run --bin game -- data/system/seattle/scenarios/downtown/weekday.bin`
    starts with a scenario (which is tied to a certain map)
  - `cargo run --bin game -- --challenge=trafficsig/tut2` starts on a particular
    challenge. See the list of aliases by passing in a bad value here.
  - `cargo run --bin game -- data/player/saves/montlake/no_edits_unnamed/00h00m20.3s.bin`
    restores an exact simulation state. Savestates are found in debug mode
    (**Control+D**) -- they're probably confusing for the normal player
    experience, so they're hidden for now.
  - `cargo run --bin game -- --tutorial=12` starts somewhere in the tutorial
  - Adding `--edits='name of edits'` starts with edits applied to the map.

## Downloading more cities

As data formats change over time, things in the `data/` directory not under
version control will get out of date. At any time, you can run
`cargo run --bin updater` from the main repository directory to update only the
files that have changed.

You can also opt into downloading updates for more cities by editing
`data/player/data.json`. In the main UI, there's a button to download more
cities that will help you manage this config file.

## Building map data

You can skip this section if you're just touching code in `game`, `widgetry`,
and `sim`.

To run all pieces of the importer, you'll need some extra dependencies:

- `osmconvert`: See <https://wiki.openstreetmap.org/wiki/Osmconvert#Download> or
  <https://github.com/interline-io/homebrew-planetutils#installation> for Mac
- `libgdal-dev`: See <https://gdal.org> if your OS package manager doesn't have
  this. If you keep hitting linking errors, then just remove
  `--features scenarios` from `import.sh`. You won't be able to build the
  Seattle scenarios.
- Standard Unix utilities: `curl`, `unzip`, `gunzip`

The first stage of the importer, `--raw`, will download input files from OSM,
King County GIS, and so on. If the mirrors are slow or the files vanish, you
could fill out `data/config` and use the `updater` described above to grab the
latest input.

Building contraction hierarchies for pathfinding occurs in the --map stage. It
can take a few minutes for larger maps. To view occasional progress updates, you
can run the importer with

    RUST_LOG="fast_paths=debug/contracted node [0-9]+0000 "

You can rerun specific stages of the importer:

- If you're modifying the initial OSM data -> RawMap conversion in
  `convert_osm`, you need `./import.sh --raw --map`.
- If you're modifying `map_model` but not the OSM -> RawMap conversion, then you
  just need `./import.sh --map`.
- If you're modifying the demand model for Seattle, you can add `--scenario` to
  regenerate.
- By default, all maps are regenerated. You can also specify a single map:
  `./import.sh --map downtown`.
- By default, Seattle is assumed as the city. You have to specify otherwise:
  `./import.sh --city=los_angeles --map downtown_la`.

You can also make the importer [import a new city](../howto/new_city.md).

## Understanding stuff

The docs listed at <https://github.com/dabreegster/abstreet#documentation>
explain things like map importing and how the traffic simulation works.

### Code organization

If you're going to dig into the code, it helps to know what all the crates are.
The most interesting crates are `map_model`, `sim`, and `game`.

Constructing the map:

- `convert_osm`: extract useful data from OpenStreetMap and other data sources,
  emit intermediate map format
- `kml`: extract shapes from KML and CSV shapefiles
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
- `map_gui`: common code to interact with `map_model` maps
- `widgetry`: a GUI and 2D OpenGL rendering library, using glium + winit +
  glutin

Common utilities:

- `abstutil`: a grab-bag timing and logging utilities
- `abstio`: Reading/writing files on native/web
- `geom`: types for GPS and map-space points, lines, angles, polylines,
  polygons, circles, durations, speeds

Other:

- `collisions`: an experimental data format for real-world collision data
- `traffic_seitan`: a bug-finding tool that randomly generates live map edits
- `tests`: integration tests
- `santa`: 15-minute Santa, an arcade game about delivering and zoning
- `parking_mapper`: a standalone tool to help map street parking in OSM
- `osm_viewer`: a standalone tool to render OSM in detail
- `fifteen_min`: a standalone tool to explore 15-minute neighborhoods
- `popdat`: use census data to produce traffic simulation input
- `traffic_signal_data`: manual timing overrides for some traffic signals
- `sumo`: interoperability with [SUMO](https://www.eclipse.org/sumo)

## Code conventions

All code is automatically formatted using
<https://github.com/rust-lang/rustfmt>; please run `cargo +nightly fmt` before
sending a PR. (You have to install the nightly toolchain just for fmt)

cargo fmt can't yet organize imports, but we follow a convention to minimize
conflict with what some IDEs do. Follow existing code to group imports: std,
external crates, other crates in the project, the current crate, then finally
any module declarations.

See the [testing strategy](testing.md) page.

## Error handling

The error handling is unfortunately inconsistent. The goal is to gracefully
degrade instead of crashing the game. If a crash does happen, make sure the logs
will have enough context to reproduce and debug. For example, giving up when
some geometry problem happens isn't ideal, but at least make sure to print the
road / agent IDs or whatever will help find the problem. It's fine to crash
during map importing, since the player won't deal with this, and loudly stopping
problems is useful. It's also fine to crash when initially constructing all of
the renderable map objects, because this crash will consistently happen at
startup-time and be noticed by somebody developing before a player gets to it.

Since almost none of the code ever needs to distinguish error cases, use
[anyhow](https://crates.io/crates/anyhow). Most of the errors generated within
A/B Street are just strings anyway; the `bail!` macro is a convenient way to
return them.

## Logging

Prefer using `info!`, `warn!`, `error!`, etc from the `log` crate rather than
`println`. Or if a `Timer` is available and you want to collect all notes
together, `timer.note`. There are still many places calling `println!`, but
we're trying to clean these up.

Adjust the log level without recompiling via the `RUST_LOG` env variable.

    RUST_LOG=debug cargo run --bin game

This can be done on a per lib basis:

    RUST_LOG=my_lib=debug cargo run --bin game

Or a module-by-module basis:

    RUST_LOG=my_lib::module=debug cargo run --bin game

You can mix and match:

    # error logging by default, except the foo:bar module at debug level
    # and the entire baz crate at info level
    RUST_LOG=error,foo::bar=debug,baz=info cargo run --bin game

For some special cases, you might want to use regex matching by specifying a
pattern with the "/":

    # only log once every 10k
    RUST_LOG="fast_paths=debug/contracted node [0-9]+0000 " mike import_la

See the [env_logger documentation](https://docs.rs/env_logger/0.8.2/env_logger/)
for more usage examples.

## Profiling

Use <https://github.com/flamegraph-rs/flamegraph>, just running it on the
binaries you build normally.
