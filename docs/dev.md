# Developer guide

## Getting started

You will first need:

- Standard dependencies: `bash`, `curl`, `unzip`, `gunzip`
- Rust, at least 1.43. https://www.rust-lang.org/tools/install

One-time setup:

1.  Download the repository:
    `git clone https://github.com/dabreegster/abstreet.git`

2.  Grab the minimal amount of data to get started:
    `./data/grab_minimal_seed_data.sh`.

3.  Run the game: `cd game; cargo run --release`

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
  - `cargo run -- --dev ../data/system/maps/downtown.bin` starts on a particular
    map
  - `cargo run ../data/system/scenarios/caphill/weekday.bin` starts with a
    scenario (which is tied to a certain map)
  - `cargo run -- --challenge=trafficsig/tut2` starts on a particular challenge.
    See the list of aliases by passing in a bad value here.
  - `cargo run ../data/player/saves/montlake/no_edits_unnamed/00h00m20.3s.bin`
    restores an exact simulation state. Savestates are found in debug mode
    (**Control+D**) -- they're probably confusing for the normal player
    experience, so they're hidden for now.
  - `cargo run -- --tutorial=12` starts somewhere in the tutorial
  - Adding `--edits='name of edits'` starts with edits applied to the map.
- All code is automatically formatted using
  https://github.com/rust-lang/rustfmt; please run `cargo +nightly fmt` before
  sending a PR. (You have to install the nightly toolchain just for fmt)
- More random notes [here](/docs/misc_dev_tricks.md)

## Building map data

You can skip this section if you're just touching code in `game`, `ezgui`, and
`sim`.

You have two options: you can seed some of the intermediate data by running
`./data/grab_all_seed_data.sh` (downloads ~1GB, expands to ~5GB), or you can
build everything totally from scratch by running
`./import.sh --raw --map --scenario`. This takes a while.

You'll need some extra dependencies:

- `osmconvert`: See https://wiki.openstreetmap.org/wiki/Osmconvert#Download
- `libgdal-dev`: See https://gdal.org/ if your OS package manager doesn't have
  this

You can rerun specific stages of the importer:

- If you're modifying the initial OSM data -> RawMap conversion in
  `convert_osm`, you need `./import.sh --raw --map`.
- If you're modifying `map_model` but not the OSM -> RawMap conversion, then you
  just need `./import.sh --map`.
- By default, all maps are regenerated. You can also specify a single map:
  `./import.sh --map downtown`.

## Understanding stuff

The docs listed at
https://github.com/dabreegster/abstreet#documentation-for-developers explain
things like map importing and how the traffic simulation works.

### Code organization

If you're going to dig into the code, it helps to know what all the crates are.
The most interesting crates are `map_model`, `sim`, and `game`.

Constructing the map:

- `convert_osm`: extract useful data from OpenStreetMap and other data sources,
  emit intermediate map format
- `gtfs`: simple library to just extract coordinates of bus stops
- `kml`: extract shapes from KML shapefiles
- `map_model`: the final representation of the map, also conversion from the
  intermediate map format into the final format
- `map_editor`: GUI for modifying geometry of maps and creating maps from
  scratch
- `importer`: tool to run the entire import pipeline

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

## Example guide for implementing a new feature

A/B Street's transit modeling only includes buses as of September 2019. If you
wanted to start modeling light rail, you'd have to touch many layers of the
code. This is a nice, hefty starter project to understand how everything works.
For now, this is just an initial list of considerations -- I haven't designed or
implemented this yet.

Poking around the .osm extracts in `data/input/osm/`, you'll see a promising
relation with `route = light_rail`. The relation points to individual points
(nodes) as stops, and segments of the track (ways). These need to be represented
in the initial version of the map, `RawMap`, and the final version, `Map`.
Stations probably coincide with existing buildings, and tracks could probably be
modeled as a special type of road. To remember the order of stations and group
everything, there's already a notion of bus route from the `gtfs` crate that
probably works. The `convert_osm` crate is the place to extract this new data
from OSM. It might be worth thinking about how the light rail line gets clipped,
since most maps won't include all of the stations -- should those maps just
terminate trains at the stations, or should trains go to and from the map
border?

Then there are some rendering questions. How should special buildings that act
as light rail stations be displayed? What about the track between stations, and
how to draw trains moving on the track? The track is sometimes underground,
sometimes at-grade with the road (like near Colombia City -- there it even has
to somehow be a part of the existing intersections!), and sometimes over the
road. How to draw it without being really visually noisy with existing stuff on
the ground? Should trains between stations even be drawn at all, or should
hovering over stations show some kind of ETA?

For modeling the movement of the trains along the track, I'd actually recommend
using the existing driving model. Tracks can be a new `LaneType` (that gets
rendered as nice train tracks, probably), and trains can be a new `VehicleType`.
This way, trains queueing happens for free. There's even existing logic to make
buses wait at bus stops and load passengers; maybe that should be extended to
load passengers from a building? How should passengers walking to the platform
be modeled and rendered -- it takes a few minutes sometimes!

Finally, you'll need to figure out how to make some trips incorporate light
rail. Pedestrian trips have the option to use transit or not -- if light rail is
modeled properly, it hopefully fits into the existing transit pathfinding and
everything, so it'll just naturally happen.
