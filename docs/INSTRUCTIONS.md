# A/B Street Instructions

General disclaimer: This is a very rough demo. The user interface is clunky, and
gameplay is not cohesively tied together yet. Please email
<dabreegster@gmail.com> or file a Github issue if you hit problems.

## Installing the game

The easiest method is to use pre-built binaries. Check
https://github.com/dabreegster/abstreet/releases for the latest version, though
I'll try to keep these links up-to-date:

- Linux:
  https://github.com/dabreegster/abstreet/releases/download/v0.1.11/abstreet_linux.zip
- Windows:
  https://github.com/dabreegster/abstreet/releases/download/v0.1.11/abstreet_windows.zip
- Mac (seems to have a HiDPI bug, text is offset -- and is a bit stale, I don't
  have a Mac to build on):
  https://github.com/dabreegster/abstreet/releases/download/v0.1.10/abstreet_mac.zip

## Running the game

Start the game by running `play_abstreet.sh` or `play_abstreet.bat`. On Windows,
you'll probably get a warning about running software from an unknown publisher.

General controls:

- Click and drag to move
- Scroll wheel or touchpad to zoom
- Menus should work as expected. Most actions have a keybinding.
- You can hover over an object and right-click to see more actions.

Things to try:

- In sandbox mode, hover over an intersection, right click, and spawn agents.
  Then you can start the simulation by pressing **space** to "resume".
- To run a realistic, full day's worth of traffic, go to sandbox mode and "reset
  sim" if needed. Then "start a scenario" and choose the "psrc" entry (this
  needs a better name). Time (shown in the top-right corner) starts at midnight.
  Things tend to get interesting around 6am. Try zooming in for details, and
  zooming out to see an overview.
- Go to edit mode (note this will reset the simulation). Pick a lane, right
  click, and change it to another type. You can also change which roads see a
  stop sign by right clicking the intersection, choosing to edit, hovering over
  a stop sign, and pressing **space** to toggle it. You can do the same for
  intersections with traffic signals.

## For developers: Compiling from source

To build, you need a Linux-like environment with `bash`, `wget`, `unzip`, etc.
You also `osmconvert` for the import script. At runtime if you want to use the
screen-capture plugin, you need `scrot`.

1.  Install Rust, at least 1.38. https://www.rust-lang.org/tools/install

2.  Download the repository:
    `git clone https://github.com/dabreegster/abstreet.git`

3.  Download all input data and build maps. Compilation times will be very slow
    the first time.
    `cd abstreet; ./import.sh && ./precompute.sh --release --disable_psrc_scenarios`.
    Alternatively, you could seed the entire `data` directory from this
    [9/21/2019 copy](https://drive.google.com/open?id=1tpHuojh1e14ZQLBhjLWf_rB6dLKy-hV7).

If you build from source, you won't have the convenient launcher scripts
referenced below. Instead:

```
cd game
cargo run --release
```

## Data source licensing

A/B Street binary releases contain pre-built maps that combine data from:

- OpenStreetMap (https://www.openstreetmap.org/copyright)
- King County metro
  (https://www.kingcounty.gov/depts/transportation/metro/travel-options/bus/app-center/terms-of-use.aspx)
- City of Seattle GIS program
  (https://www.opendatacommons.org/licenses/pddl/1.0/)
- https://github.com/seattleio/seattle-boundaries-data
  (https://creativecommons.org/publicdomain/zero/1.0/)
- DejaVuSans.ttf (https://dejavu-fonts.github.io/License.html)
- Puget Sound Regional Council
  (https://www.psrc.org/activity-based-travel-model-soundcast)
- http://www.textures4photoshop.com for textures
