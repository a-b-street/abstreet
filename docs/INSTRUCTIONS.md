# A/B Street Instructions

General disclaimer: This is a very rough demo. The user interface is clunky, and
gameplay is not cohesively tied together yet. Please email
<dabreegster@gmail.com> or file a Github issue if you hit problems.

## Installing the game from pre-built binaries

- Windows:
  https://github.com/dabreegster/abstreet/releases/download/v0.1.16/abstreet_windows.zip
- Mac:
  https://github.com/dabreegster/abstreet/releases/download/v0.1.16/abstreet_mac.zip
- Linux:
  https://github.com/dabreegster/abstreet/releases/download/v0.1.16/abstreet_linux.zip

Unzip the folder, then run `play_abstreet.sh` or `play_abstreet.bat`. On
Windows, you'll probably get a warning about running software from an unknown
publisher.

## Playing the game

General controls:

- Click and drag to move
- Scroll wheel or touchpad to zoom
- Menus should work as expected. Most actions have a keybinding.
- You can hover over an object and right-click to see more actions.

Things to try:

- In sandbox mode, hover over an intersection, right click, and spawn agents.
  Then you can start the simulation by pressing **space** or clicking the icon.
- To run a realistic, full day's worth of traffic, go to sandbox mode, then
  "start a scenario" (hotkey **s**) and choose the "weekday_typical_traffic"
  entry. Time (shown in the top-right corner) starts at midnight. Things tend to
  get interesting around 6am -- use the speed controls in the top-left. Try
  zooming in for details, and zooming out to see an overview.
- Go to edit mode (note this will reset the simulation). Pick a lane, right
  click, and change it to another type. You can also change which roads see a
  stop sign by right clicking the intersection, choosing to edit, hovering over
  a stop sign, and pressing **space** to toggle it. You can do the same for
  intersections with traffic signals.
- Go back to the main menu and pick a challenge. Should be self-explanatory from
  there -- leave me feedbck if not.

## For developers: Compiling from source

You will first need:

- Standard dependencies: `bash`, `curl`, `unzip`, `gunzip`
- `osmconvert`: See https://wiki.openstreetmap.org/wiki/Osmconvert#Download
- Rust, at least 1.38. https://www.rust-lang.org/tools/install

One-time setup:

1.  Download the repository:
    `git clone https://github.com/dabreegster/abstreet.git`

2.  Download all input data and build maps. Compilation times will be very slow
    the first time. `cd abstreet; ./import.sh && ./precompute.sh --release`

3.  Run the game: `cd game; cargo run --release`

### Development tips

- You don't need to rerun `./import.sh` nd `./precompute.sh` most of the time.
  - If you're just touching code in `game`, `sim`, and `ezgui`, you can just
    `cargo run` from `game`
  - If you're modifying the initial OSM data -> RawMap conversion in
    `convert_osm`, then you do need to rerun `./import.sh` and `precompute.sh`
    to regenerate the map.
  - If you're modifying `map_model` but not the OSM -> RawMap conversion, then
    you can just do `precompute.sh`.
  - Both of those scripts can just regenerate a single map, which is much
    faster: `./import.sh caphill; ./precompute.sh caphill`
- Compile faster by just doing `cargo run`. The executable will have debug stack
  traces and run more slowly. You can do `cargo run --release` to build in
  optimized release mode; compilation will be slower, but the executable much
  faster.
- To add some extra debug modes to the game, `cargo run -- --dev` or press
  Control+S to toggle in-game
- All code is automatically formatted using
  https://github.com/rust-lang/rustfmt; please run `cargo fmt` before sending a
  PR.

## Data source licensing

A/B Street binary releases contain pre-built maps that combine data from:

- OpenStreetMap (https://www.openstreetmap.org/copyright)
- King County metro
  (https://www.kingcounty.gov/depts/transportation/metro/travel-options/bus/app-center/terms-of-use.aspx)
- City of Seattle GIS program
  (https://www.opendatacommons.org/licenses/pddl/1.0/)
- https://github.com/seattleio/seattle-boundaries-data
  (https://creativecommons.org/publicdomain/zero/1.0/)
- Puget Sound Regional Council
  (https://www.psrc.org/activity-based-travel-model-soundcast)

Other binary data bundled in:

- DejaVuSans.ttf (https://dejavu-fonts.github.io/License.html)
- Roboto-Regular.ttf (https://fonts.google.com/specimen/Roboto, Apache license)
- http://www.textures4photoshop.com for textures
- Icons from https://thenounproject.com/aiga-icons/,
  https://thenounproject.com/sakchai.ruankam, https://thenounproject.com/wilmax
